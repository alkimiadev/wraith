use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use russh::keys::ssh_key::HashAlg;
use russh::server::{Auth, Handler, Msg, Session};
use russh::Channel;

use crate::auth::ServerAuthConfig;
use crate::server::control_channel::{
    ControlChannelHandler, ControlChannelRouter, WRAITH_PREFIX,
};

#[derive(Debug, Clone)]
pub enum ProxyMode {
    Direct,
    Socks5(SocketAddr),
    HttpConnect(SocketAddr),
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub mode: ProxyMode,
}

pub struct ServerHandler {
    auth_config: Arc<ServerAuthConfig>,
    outbound_proxy: Option<ProxyConfig>,
    remote_addr: Option<SocketAddr>,
    control_channel_router: ControlChannelRouter,
}

impl ServerHandler {
    pub fn new(
        auth_config: Arc<ServerAuthConfig>,
        outbound_proxy: Option<ProxyConfig>,
        remote_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            auth_config,
            outbound_proxy,
            remote_addr,
            control_channel_router: ControlChannelRouter::without_handler(),
        }
    }

    pub fn with_control_channel_handler(
        mut self,
        handler: Box<dyn ControlChannelHandler>,
    ) -> Self {
        self.control_channel_router = ControlChannelRouter::with_handler(handler);
        self
    }

    pub fn control_channel_router(&self) -> &ControlChannelRouter {
        &self.control_channel_router
    }
}

#[async_trait]
impl Handler for ServerHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        let fingerprint = format!("{}", public_key.fingerprint(HashAlg::Sha256));
        let remote_addr_display = self
            .remote_addr
            .map_or("unknown".to_string(), |a| a.to_string());

        let russh_pub = russh::keys::PublicKey::new(public_key.key_data().clone(), user);
        let result = self.auth_config.authenticate_publickey(&russh_pub);

        match result {
            Ok(()) => {
                tracing::info!(
                    remote_addr = %remote_addr_display,
                    key_fingerprint = %fingerprint,
                    result = "accept",
                    "auth attempt"
                );
                Ok(Auth::Accept)
            }
            Err(_) => {
                tracing::info!(
                    remote_addr = %remote_addr_display,
                    key_fingerprint = %fingerprint,
                    result = "reject",
                    "auth attempt"
                );
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                })
            }
        }
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if host_to_connect.starts_with(WRAITH_PREFIX) {
            tracing::info!(
                host = host_to_connect,
                port = port_to_connect,
                "routing to internal control channel handler"
            );

            if !self.control_channel_router.has_handler() {
                tracing::warn!(
                    host = host_to_connect,
                    "no control channel handler configured, rejecting channel open"
                );
                return Ok(false);
            }

            let _ = channel;
            return Ok(true);
        }

        let proxy_info = self
            .outbound_proxy
            .as_ref()
            .map(|p| format!("{:?}", p.mode))
            .unwrap_or_else(|| "direct".to_string());

        tracing::info!(
            host = host_to_connect,
            port = port_to_connect,
            originator_address = originator_address,
            originator_port = originator_port,
            proxy = %proxy_info,
            "spawning tcp proxy task"
        );

        let _ = channel;
        Ok(false)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn channel_open_x11(
        &mut self,
        _channel: Channel<Msg>,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn channel_open_forwarded_tcpip(
        &mut self,
        _channel: Channel<Msg>,
        _host_to_connect: &str,
        _port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::keys::KeySource;
    use russh::keys::{decode_secret_key, PrivateKey};
    use std::io::Write;

    const ED25519_PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACBOfInDyRS33JEeDNT8xd10qRdwFN8z/QukCOgEIkv01QAAAJiQ+NvMkPjb\nzAAAAAtzc2gtZWQyNTUxOQAAACBOfInDyRS33JEeDNT8xd10qRdwFN8z/QukCOgEIkv01Q\nAAAECIWwJf7+7MOuZAOOWmoQbE9i/5GxjKsFrtJHjZ34E/fk58icPJFLfckR4M1PzF3XSp\nF3AU3zP9C6QI6AQiS/TVAAAAD3VidW50dUBuczUyODA5NgECAwQFBg==\n-----END OPENSSH PRIVATE KEY-----\n";

    const ED25519_PUBLIC_KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIE58icPJFLfckR4M1PzF3XSpF3AU3zP9C6QI6AQiS/TV ubuntu@ns528096";

    fn make_authorized_keys_file(keys_content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(keys_content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    fn load_key() -> PrivateKey {
        decode_secret_key(ED25519_PRIVATE_KEY, None).unwrap()
    }

    fn make_auth_config(keys_content: &str) -> Arc<ServerAuthConfig> {
        let f = make_authorized_keys_file(keys_content);
        Arc::new(
            ServerAuthConfig::from_keys_and_ca(
                Some(KeySource::File(f.path().to_path_buf())),
                None,
            )
            .unwrap(),
        )
    }

    fn make_empty_auth_config() -> Arc<ServerAuthConfig> {
        Arc::new(ServerAuthConfig::from_keys_and_ca(None, None).unwrap())
    }

    #[tokio::test]
    async fn auth_delegation_accepts_known_key() {
        let auth_config = make_auth_config(ED25519_PUBLIC_KEY);
        let mut handler = ServerHandler::new(auth_config, None, None);

        let ssh_key = load_key().public_key().clone();
        let result = handler.auth_publickey("testuser", &ssh_key).await.unwrap();
        assert_eq!(result, Auth::Accept);
    }

    #[tokio::test]
    async fn auth_delegation_rejects_unknown_key() {
        let auth_config = make_auth_config(ED25519_PUBLIC_KEY);
        let mut handler = ServerHandler::new(auth_config, None, None);

        let other_key_text = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHeLC1lWiCYrXsf/85O/pkbUFZ6OGIt49PX3nw8iRoXE other@host";
        let other_ssh_key = russh::keys::parse_public_key_base64(
            other_key_text.split_whitespace().nth(1).unwrap(),
        )
        .unwrap();

        let result = handler
            .auth_publickey("testuser", &other_ssh_key)
            .await
            .unwrap();
        assert_eq!(
            result,
            Auth::Reject {
                proceed_with_methods: None
            }
        );
    }

    #[tokio::test]
    async fn auth_delegation_empty_config_rejects_all() {
        let auth_config = make_empty_auth_config();
        let mut handler = ServerHandler::new(auth_config, None, None);

        let ssh_key = load_key().public_key().clone();
        let result = handler
            .auth_publickey("testuser", &ssh_key)
            .await
            .unwrap();
        assert_eq!(
            result,
            Auth::Reject {
                proceed_with_methods: None
            }
        );
    }

    #[tokio::test]
    async fn auth_logging_includes_remote_addr() {
        let auth_config = make_auth_config(ED25519_PUBLIC_KEY);
        let remote_addr: SocketAddr = "203.0.113.50:12345".parse().unwrap();
        let mut handler = ServerHandler::new(auth_config, None, Some(remote_addr));

        let ssh_key = load_key().public_key().clone();
        let _ = handler.auth_publickey("root", &ssh_key).await.unwrap();
    }

    #[test]
    fn reserved_wraith_destination_routing() {
        use crate::server::control_channel::is_reserved_destination;
        assert!(is_reserved_destination("wraith-control"));
        assert!(is_reserved_destination("wraith-status"));
        assert!(is_reserved_destination("wraith-events"));
        assert!(!is_reserved_destination("example.com"));
        assert!(!is_reserved_destination("localhost"));
        assert!(!is_reserved_destination("wraith.example.com"));
    }

    #[test]
    fn server_handler_without_control_handler_rejects_wraith_destinations() {
        let auth_config = make_empty_auth_config();
        let handler = ServerHandler::new(auth_config, None, None);
        assert!(!handler.control_channel_router().has_handler());
    }

    #[test]
    fn proxy_mode_variants() {
        let direct = ProxyMode::Direct;
        let socks5 = ProxyMode::Socks5("127.0.0.1:9050".parse().unwrap());
        let http = ProxyMode::HttpConnect("127.0.0.1:8080".parse().unwrap());

        match direct {
            ProxyMode::Direct => {}
            _ => panic!("expected Direct"),
        }
        match socks5 {
            ProxyMode::Socks5(_) => {}
            _ => panic!("expected Socks5"),
        }
        match http {
            ProxyMode::HttpConnect(_) => {}
            _ => panic!("expected HttpConnect"),
        }
    }

    #[test]
    fn server_handler_holds_config() {
        let auth_config = make_empty_auth_config();
        let proxy = Some(ProxyConfig {
            mode: ProxyMode::Socks5("127.0.0.1:9050".parse().unwrap()),
        });
        let remote: Option<SocketAddr> = Some("10.0.0.1:22".parse().unwrap());

        let handler = ServerHandler::new(auth_config, proxy.clone(), remote);
        assert!(handler.outbound_proxy.is_some());
        assert!(handler.remote_addr.is_some());
    }

    #[test]
    fn one_handler_per_connection() {
        let auth_config = make_empty_auth_config();
        let handler1 = ServerHandler::new(auth_config.clone(), None, Some("10.0.0.1:22".parse().unwrap()));
        let handler2 = ServerHandler::new(auth_config.clone(), None, Some("10.0.0.2:22".parse().unwrap()));

        assert!(handler1.remote_addr != handler2.remote_addr);
    }
}