use std::net::SocketAddr;
use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use russh::server;
use russh::Channel;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use wraith_core::auth::keys::KeySource;
use wraith_core::auth::server_auth::ServerAuthConfig;
use wraith_core::server::rate_limit::{AuthAttemptLimiter, ConnectionRateLimiter};
use wraith_core::server::serve::{ServeOptions, ServeTransportMode, Server};
use wraith_core::transport::{TcpAcceptor, TransportAcceptor};

#[napi(object)]
pub struct WraithServeOptions {
    pub transport: String,
    pub host_key: Option<Either<String, Buffer>>,
    pub authorized_keys: Option<Either<String, Buffer>>,
    pub cert_authority: Option<Either<String, Buffer>>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub acme_domain: Option<String>,
    pub listen: Option<String>,
    pub iroh_relay: Option<String>,
}

fn resolve_key_source(
    key: &Option<Either<String, Buffer>>,
    field: &str,
) -> napi::Result<KeySource> {
    match key {
        None => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "{} is required: provide a file path (string) or key data (Buffer)",
                field
            ),
        )),
        Some(Either::A(path)) => Ok(KeySource::File(path.into())),
        Some(Either::B(buf)) => Ok(KeySource::Memory(buf.to_vec())),
    }
}

fn resolve_optional_key_source(key: &Option<Either<String, Buffer>>) -> Option<KeySource> {
    match key {
        None => None,
        Some(Either::A(path)) => Some(KeySource::File(path.into())),
        Some(Either::B(buf)) => Some(KeySource::Memory(buf.to_vec())),
    }
}

fn parse_addr(addr_str: &str) -> napi::Result<SocketAddr> {
    addr_str.parse().map_err(|e| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid address '{}': {}", addr_str, e),
        )
    })
}

#[napi(object)]
pub struct ConnectionInfo {
    pub remote_addr: Option<String>,
    pub transport_kind: String,
}

#[napi]
pub struct WraithServerStream {
    read: Arc<Mutex<tokio::io::ReadHalf<russh::ChannelStream<server::Msg>>>>,
    write: Arc<Mutex<tokio::io::WriteHalf<russh::ChannelStream<server::Msg>>>>,
}

#[napi]
impl WraithServerStream {
    #[napi]
    pub async fn read(&self, size: u32) -> napi::Result<Buffer> {
        let mut buf = vec![0u8; size as usize];
        let mut guard = self.read.lock().await;
        let n = guard.read(&mut buf).await.map_err(|e| {
            napi::Error::new(napi::Status::GenericFailure, format!("read failed: {}", e))
        })?;
        if n == 0 {
            return Ok(Vec::<u8>::new().into());
        }
        buf.truncate(n);
        Ok(buf.into())
    }

    #[napi]
    pub async fn write(&self, data: Buffer) -> napi::Result<()> {
        let mut guard = self.write.lock().await;
        guard.write_all(&data).await.map_err(|e| {
            napi::Error::new(napi::Status::GenericFailure, format!("write failed: {}", e))
        })?;
        Ok(())
    }

    #[napi]
    pub async fn close(&self) -> napi::Result<()> {
        let mut guard = self.write.lock().await;
        guard.shutdown().await.map_err(|e| {
            napi::Error::new(napi::Status::GenericFailure, format!("close failed: {}", e))
        })
    }
}

struct NapiServerHandler {
    auth_config: Arc<ServerAuthConfig>,
    remote_addr: Option<SocketAddr>,
    connection_limiter: Arc<ConnectionRateLimiter>,
    connection_allowed: bool,
    auth_limiter: AuthAttemptLimiter,
    channel_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Channel<server::Msg>>>>>,
}

impl NapiServerHandler {
    fn new(
        auth_config: Arc<ServerAuthConfig>,
        remote_addr: Option<SocketAddr>,
        connection_limiter: Arc<ConnectionRateLimiter>,
        max_auth_attempts: usize,
        channel_sender: Arc<
            Mutex<Option<tokio::sync::mpsc::UnboundedSender<Channel<server::Msg>>>>,
        >,
    ) -> Self {
        let allowed = if let Some(addr) = remote_addr {
            let ip = addr.ip();
            if connection_limiter.check(ip) {
                connection_limiter.on_connect(ip);
                true
            } else {
                false
            }
        } else {
            true
        };

        Self {
            auth_config,
            remote_addr,
            connection_limiter,
            connection_allowed: allowed,
            auth_limiter: AuthAttemptLimiter::new(max_auth_attempts),
            channel_sender,
        }
    }

    fn is_connection_allowed(&self) -> bool {
        self.connection_allowed
    }
}

impl Drop for NapiServerHandler {
    fn drop(&mut self) {
        if let Some(addr) = self.remote_addr {
            if self.connection_allowed {
                self.connection_limiter.on_disconnect(addr.ip());
            }
        }
    }
}

#[async_trait::async_trait]
impl russh::server::Handler for NapiServerHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<russh::server::Auth, Self::Error> {
        if !self.auth_limiter.check() {
            return Ok(russh::server::Auth::Reject {
                proceed_with_methods: None,
            });
        }

        let russh_pub = russh::keys::PublicKey::new(public_key.key_data().clone(), user);
        let result = self.auth_config.authenticate_publickey(&russh_pub);

        match result {
            Ok(()) => Ok(russh::server::Auth::Accept),
            Err(_) => {
                self.auth_limiter.on_failure();
                Ok(russh::server::Auth::Reject {
                    proceed_with_methods: None,
                })
            }
        }
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host_to_connect: &str,
        _port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::server::Session,
    ) -> std::result::Result<bool, Self::Error> {
        if host_to_connect.starts_with("wraith-") {
            let guard = self.channel_sender.lock().await;
            if let Some(ref tx) = *guard {
                let _ = tx.send(channel);
            }
            return Ok(true);
        }

        let _ = channel;
        Ok(false)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<server::Msg>,
        _session: &mut russh::server::Session,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(false)
    }

    async fn channel_open_x11(
        &mut self,
        _channel: Channel<server::Msg>,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::server::Session,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(false)
    }

    async fn channel_open_forwarded_tcpip(
        &mut self,
        _channel: Channel<server::Msg>,
        _host_to_connect: &str,
        _port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::server::Session,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(false)
    }
}

type ServerTsfn = ThreadsafeFunction<ConnectionEventWrapper, (), ConnectionEventWrapper>;

#[napi]
pub struct WraithServer {
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    listen_addr: String,
    on_connection_tsfn: Arc<Mutex<Option<ServerTsfn>>>,
}

struct ConnectionEventWrapper {
    stream: WraithServerStream,
    info: ConnectionInfo,
}

impl ToNapiValue for ConnectionEventWrapper {
    unsafe fn to_napi_value(
        env: napi::sys::napi_env,
        val: Self,
    ) -> napi::Result<napi::sys::napi_value> {
        let mut raw_obj: napi::sys::napi_value = std::ptr::null_mut();
        napi::check_status!(
            napi::sys::napi_create_object(env, &mut raw_obj),
            "Failed to create object"
        )?;

        let stream_val = <WraithServerStream as ToNapiValue>::to_napi_value(env, val.stream)?;
        let key_stream = std::ffi::CString::new("stream").unwrap();
        napi::check_status!(
            napi::sys::napi_set_named_property(env, raw_obj, key_stream.as_ptr(), stream_val),
            "Failed to set stream property"
        )?;

        let info_val = <ConnectionInfo as ToNapiValue>::to_napi_value(env, val.info)?;
        let key_info = std::ffi::CString::new("info").unwrap();
        napi::check_status!(
            napi::sys::napi_set_named_property(env, raw_obj, key_info.as_ptr(), info_val),
            "Failed to set info property"
        )?;

        Ok(raw_obj)
    }
}

impl TypeName for ConnectionEventWrapper {
    fn type_name() -> &'static str {
        "ConnectionEventWrapper"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Object
    }
}

impl ValidateNapiValue for ConnectionEventWrapper {}

#[napi]
impl WraithServer {
    #[napi]
    pub async fn close(&self) -> napi::Result<()> {
        let _ = self.shutdown_tx.send(true);
        Ok(())
    }

    #[napi(ts_return_type = "void")]
    pub fn on_connection(&self, callback: Function<(), ()>) -> napi::Result<()> {
        let tsfn = callback
            .build_threadsafe_function::<ConnectionEventWrapper>()
            .callee_handled::<true>()
            .build_callback(|ctx| Ok(ctx.value))?;

        let holder = self.on_connection_tsfn.clone();
        *holder.blocking_lock() = Some(tsfn);
        Ok(())
    }

    #[napi(getter)]
    pub fn listen_addr(&self) -> napi::Result<String> {
        Ok(self.listen_addr.clone())
    }
}

#[napi]
pub async fn serve(options: WraithServeOptions) -> napi::Result<WraithServer> {
    let host_key_source = resolve_key_source(&options.host_key, "hostKey")?;
    let authorized_keys_source = resolve_optional_key_source(&options.authorized_keys);
    let cert_authority_source = resolve_optional_key_source(&options.cert_authority);

    let transport_mode = match options.transport.to_lowercase().as_str() {
        "tcp" => ServeTransportMode::Tcp,
        "tls" => ServeTransportMode::Tls,
        "iroh" => ServeTransportMode::Iroh,
        other => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("unknown transport '{}'; expected tcp, tls, or iroh", other),
            ));
        }
    };

    let listen_addr_str = options.listen.as_deref().unwrap_or("0.0.0.0:22");

    let mut serve_opts = ServeOptions::new(host_key_source.clone())
        .transport_mode(transport_mode.clone())
        .listen_addr(listen_addr_str);

    if let Some(aks) = authorized_keys_source.clone() {
        serve_opts = serve_opts.authorized_keys(aks);
    }
    if let Some(cas) = cert_authority_source.clone() {
        serve_opts = serve_opts.cert_authority(cas);
    }
    if let Some(ref cert) = options.tls_cert {
        serve_opts = serve_opts.tls_cert(cert);
    }
    if let Some(ref key) = options.tls_key {
        serve_opts = serve_opts.tls_key(key);
    }
    if let Some(ref domain) = options.acme_domain {
        serve_opts = serve_opts.acme_domain(domain);
    }
    if let Some(ref relay) = options.iroh_relay {
        serve_opts = serve_opts.iroh_relay(relay);
    }

    let _core_server = Server::new(serve_opts).map_err(|e| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("server config error: {}", e),
        )
    })?;

    let shutdown_tx = _core_server.shutdown_sender();

    match transport_mode {
        ServeTransportMode::Tcp => {
            let addr = parse_addr(listen_addr_str)?;
            let acceptor = TcpAcceptor::bind(addr).await.map_err(|e| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!("tcp bind failed: {}", e),
                )
            })?;
            let actual_listen = acceptor.listen_addr().to_string();

            let auth_config = Arc::new(
                ServerAuthConfig::from_keys_and_ca(authorized_keys_source, cert_authority_source)
                    .map_err(|e| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("auth config error: {}", e),
                    )
                })?,
            );

            let private_key =
                wraith_core::auth::keys::load_private_key(host_key_source).map_err(|e| {
                    napi::Error::new(napi::Status::InvalidArg, format!("host key error: {}", e))
                })?;

            let config = Arc::new(server::Config {
                keys: vec![private_key],
                ..Default::default()
            });

            let connection_limiter = Arc::new(ConnectionRateLimiter::new(0));
            let shutdown_rx = shutdown_tx.subscribe();
            let tsfn_holder: Arc<Mutex<Option<ServerTsfn>>> = Arc::new(Mutex::new(None));

            let tsfn_for_loop = tsfn_holder.clone();

            tokio::spawn(async move {
                run_accept_loop(
                    acceptor,
                    config,
                    auth_config,
                    connection_limiter,
                    shutdown_rx,
                    tsfn_for_loop,
                )
                .await;
            });

            Ok(WraithServer {
                shutdown_tx,
                listen_addr: actual_listen,
                on_connection_tsfn: tsfn_holder,
            })
        }
        ServeTransportMode::Tls => Err(napi::Error::new(
            napi::Status::GenericFailure,
            "TLS transport is not yet supported in napi serve()".to_string(),
        )),
        ServeTransportMode::Iroh => Err(napi::Error::new(
            napi::Status::GenericFailure,
            "iroh transport is not yet supported in napi serve()".to_string(),
        )),
    }
}

async fn run_accept_loop<A>(
    acceptor: A,
    config: Arc<server::Config>,
    auth_config: Arc<ServerAuthConfig>,
    connection_limiter: Arc<ConnectionRateLimiter>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    tsfn_holder: Arc<Mutex<Option<ServerTsfn>>>,
) where
    A: TransportAcceptor + Send + Sync + 'static,
{
    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        let accept_result = tokio::select! {
            result = acceptor.accept() => result,
            _ = shutdown_rx.changed() => break,
        };

        let (stream, info) = match accept_result {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let remote_addr = info.remote_addr;
        let (channel_tx, mut channel_rx) =
            tokio::sync::mpsc::unbounded_channel::<Channel<server::Msg>>();
        let channel_sender = Arc::new(Mutex::new(Some(channel_tx)));

        let handler = NapiServerHandler::new(
            Arc::clone(&auth_config),
            remote_addr,
            Arc::clone(&connection_limiter),
            10,
            channel_sender,
        );

        if !handler.is_connection_allowed() {
            continue;
        }

        let config = Arc::clone(&config);
        let tsfn_holder = tsfn_holder.clone();
        let remote_addr_str = remote_addr.map(|a| a.to_string());
        let transport_kind_str = "tcp".to_string();

        tokio::spawn(async move {
            let running = match server::run_stream(config, stream, handler).await {
                Ok(r) => r,
                Err(_) => return,
            };

            loop {
                let channel = channel_rx.recv().await;
                match channel {
                    Some(ch) => {
                        let channel_stream = ch.into_stream();
                        let (read_half, write_half) = tokio::io::split(channel_stream);
                        let server_stream = WraithServerStream {
                            read: Arc::new(Mutex::new(read_half)),
                            write: Arc::new(Mutex::new(write_half)),
                        };

                        let conn_info = ConnectionInfo {
                            remote_addr: remote_addr_str.clone(),
                            transport_kind: transport_kind_str.clone(),
                        };

                        let event = ConnectionEventWrapper {
                            stream: server_stream,
                            info: conn_info,
                        };

                        let tsfn_guard = tsfn_holder.lock().await;
                        if let Some(ref tsfn) = *tsfn_guard {
                            let _ = tsfn.call(Ok(event), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                    }
                    None => break,
                }
            }

            let _ = running.await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::server::Handler;

    const ED25519_PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACBOfInDyRS33JEeDNT8xd10qRdwFN8z/QukCOgEIkv01QAAAJiQ+NvMkPjb\nzAAAAAtzc2gtZWQyNTUxOQAAACBOfInDyRS33JEeDNT8xd10qRdwFN8z/QukCOgEIkv01Q\nAAAECIWwJf7+7MOuZAOOWmoQbE9i/5GxjKsFrtJHjZ34E/fk58icPJFLfckR4M1PzF3XSp\nF3AU3zP9C6QI6AQiS/TVAAAAD3VidW50dUBuczUyODA5NgECAwQFBg==\n-----END OPENSSH PRIVATE KEY-----\n";

    #[test]
    fn resolve_key_source_file_path() {
        let key = Some(Either::<String, Buffer>::A("/path/to/key".to_string()));
        let result = resolve_key_source(&key, "hostKey");
        assert!(result.is_ok());
        match result.unwrap() {
            KeySource::File(p) => assert_eq!(p.to_str(), Some("/path/to/key")),
            _ => panic!("expected File variant"),
        }
    }

    #[test]
    fn resolve_key_source_buffer() {
        let key = Some(Either::<String, Buffer>::B(Buffer::from(
            ED25519_PRIVATE_KEY.as_bytes().to_vec(),
        )));
        let result = resolve_key_source(&key, "hostKey");
        assert!(result.is_ok());
        match result.unwrap() {
            KeySource::Memory(data) => assert!(!data.is_empty()),
            _ => panic!("expected Memory variant"),
        }
    }

    #[test]
    fn resolve_key_source_missing() {
        let key: Option<Either<String, Buffer>> = None;
        assert!(resolve_key_source(&key, "hostKey").is_err());
    }

    #[test]
    fn resolve_optional_key_source_none() {
        let key: Option<Either<String, Buffer>> = None;
        assert!(resolve_optional_key_source(&key).is_none());
    }

    #[test]
    fn resolve_optional_key_source_file() {
        let key = Some(Either::<String, Buffer>::A("/path/to/keys".to_string()));
        let result = resolve_optional_key_source(&key);
        assert!(result.is_some());
        match result.unwrap() {
            KeySource::File(p) => assert_eq!(p.to_str(), Some("/path/to/keys")),
            _ => panic!("expected File variant"),
        }
    }

    #[test]
    fn resolve_optional_key_source_buffer() {
        let key = Some(Either::<String, Buffer>::B(Buffer::from(
            b"keydata".to_vec(),
        )));
        let result = resolve_optional_key_source(&key);
        assert!(result.is_some());
        match result.unwrap() {
            KeySource::Memory(data) => assert_eq!(data, b"keydata".to_vec()),
            _ => panic!("expected Memory variant"),
        }
    }

    #[test]
    fn parse_addr_valid() {
        let addr = parse_addr("127.0.0.1:22");
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap().port(), 22);
    }

    #[test]
    fn parse_addr_invalid() {
        assert!(parse_addr("not-an-address").is_err());
    }

    #[test]
    fn connection_info_fields() {
        let info = ConnectionInfo {
            remote_addr: Some("127.0.0.1:12345".to_string()),
            transport_kind: "tcp".to_string(),
        };
        assert_eq!(info.remote_addr, Some("127.0.0.1:12345".to_string()));
        assert_eq!(info.transport_kind, "tcp");
    }

    #[test]
    fn napi_server_handler_allows_connection() {
        let auth_config = Arc::new(ServerAuthConfig::from_keys_and_ca(None, None).unwrap());
        let (tx, _) = tokio::sync::mpsc::unbounded_channel::<Channel<server::Msg>>();
        let handler = NapiServerHandler::new(
            auth_config,
            None,
            Arc::new(ConnectionRateLimiter::new(0)),
            10,
            Arc::new(Mutex::new(Some(tx))),
        );
        assert!(handler.is_connection_allowed());
    }

    #[tokio::test]
    async fn napi_server_handler_rejects_unknown_key() {
        let auth_config = Arc::new(ServerAuthConfig::from_keys_and_ca(None, None).unwrap());
        let (tx, _) = tokio::sync::mpsc::unbounded_channel::<Channel<server::Msg>>();
        let mut handler = NapiServerHandler::new(
            auth_config,
            None,
            Arc::new(ConnectionRateLimiter::new(0)),
            10,
            Arc::new(Mutex::new(Some(tx))),
        );

        let test_key_str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHeLC1lWiCYrXsf/85O/pkbUFZ6OGIt49PX3nw8iRoXE test@host";
        let public_key =
            russh::keys::parse_public_key_base64(test_key_str.split_whitespace().nth(1).unwrap())
                .unwrap();

        let result = handler
            .auth_publickey("testuser", &public_key)
            .await
            .unwrap();
        assert_eq!(
            result,
            russh::server::Auth::Reject {
                proceed_with_methods: None
            }
        );
    }

    #[test]
    fn napi_server_handler_connection_limiter() {
        let limiter = Arc::new(ConnectionRateLimiter::new(1));
        let auth_config = Arc::new(ServerAuthConfig::from_keys_and_ca(None, None).unwrap());
        let (tx, _) = tokio::sync::mpsc::unbounded_channel::<Channel<server::Msg>>();
        let addr: SocketAddr = "10.0.0.1:22".parse().unwrap();

        let h1 = NapiServerHandler::new(
            auth_config.clone(),
            Some(addr),
            limiter.clone(),
            10,
            Arc::new(Mutex::new(Some(tx.clone()))),
        );
        assert!(h1.is_connection_allowed());

        let h2 = NapiServerHandler::new(
            auth_config,
            Some(addr),
            limiter,
            10,
            Arc::new(Mutex::new(Some(tx))),
        );
        assert!(!h2.is_connection_allowed());

        drop(h1);

        let h3 = NapiServerHandler::new(
            Arc::new(ServerAuthConfig::from_keys_and_ca(None, None).unwrap()),
            Some(addr),
            Arc::new(ConnectionRateLimiter::new(1)),
            10,
            Arc::new(Mutex::new(None)),
        );
        assert!(h3.is_connection_allowed());
    }
}
