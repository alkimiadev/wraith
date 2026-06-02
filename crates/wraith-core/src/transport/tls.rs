use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, ServerConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{client::TlsStream as ClientTlsStream, TlsAcceptor as TokioTlsAcceptor, TlsConnector};

use super::{Transport, TransportAcceptor, TransportInfo, TransportKind};

pub struct TlsTransport {
    addr: SocketAddr,
    tls_server_name: Option<String>,
    insecure: bool,
    root_cert: Option<CertificateDer<'static>>,
}

impl TlsTransport {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            tls_server_name: None,
            insecure: false,
            root_cert: None,
        }
    }

    pub fn with_server_name(mut self, name: impl Into<String>) -> Self {
        self.tls_server_name = Some(name.into());
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    pub fn with_root_cert(mut self, cert: CertificateDer<'static>) -> Self {
        self.root_cert = Some(cert);
        self
    }

    fn build_client_config(&self) -> Result<ClientConfig> {
        if self.insecure {
            let config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth();
            return Ok(config);
        }

        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        if let Some(ref cert) = self.root_cert {
            root_store.add(cert.clone())?;
        }

        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        Ok(config)
    }

    fn resolve_server_name(&self) -> Result<ServerName<'static>> {
        let name = match &self.tls_server_name {
            Some(n) => n.clone(),
            None => self.addr.ip().to_string(),
        };
        ServerName::try_from(name.clone())
            .map_err(move |e| anyhow!("invalid server name '{}': {}", name, e))
    }
}

#[async_trait]
impl Transport for TlsTransport {
    type Stream = ClientTlsStream<TcpStream>;

    async fn connect(&self) -> Result<Self::Stream> {
        let tcp_stream = TcpStream::connect(self.addr).await?;
        let config = self.build_client_config()?;
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = self.resolve_server_name()?;
        let tls_stream = connector.connect(server_name, tcp_stream).await?;
        Ok(tls_stream)
    }

    fn describe(&self) -> String {
        format!("tls://{}", self.addr)
    }
}

#[derive(Debug)]
pub struct AcmeConfig {
    pub domain: String,
}

pub struct TlsAcceptor {
    listener: TcpListener,
    listen_addr: SocketAddr,
    #[allow(dead_code)]
    server_config: Arc<ServerConfig>,
    tokio_acceptor: TokioTlsAcceptor,
}

impl TlsAcceptor {
    pub async fn bind(
        addr: SocketAddr,
        tls_certs: Vec<CertificateDer<'static>>,
        tls_key: PrivateKeyDer<'static>,
        _acme_config: Option<AcmeConfig>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let listen_addr = listener.local_addr()?;

        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(tls_certs, tls_key)?;

        let server_config = Arc::new(server_config);
        let tokio_acceptor = TokioTlsAcceptor::from(server_config.clone());

        Ok(Self {
            listener,
            listen_addr,
            server_config,
            tokio_acceptor,
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }
}

#[async_trait]
impl TransportAcceptor for TlsAcceptor {
    type Stream = tokio_rustls::server::TlsStream<TcpStream>;

    async fn accept(&self) -> Result<(Self::Stream, TransportInfo)> {
        let (tcp_stream, remote_addr) = self.listener.accept().await?;
        let tls_stream = self.tokio_acceptor.accept(tcp_stream).await?;

        let server_name = tls_stream
            .get_ref()
            .1
            .server_name()
            .map(|s| s.to_string());

        let info = TransportInfo {
            remote_addr: Some(remote_addr),
            transport_kind: TransportKind::Tls { server_name },
        };

        Ok((tls_stream, info))
    }
}

#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _doc: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _doc: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, KeyPair};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn generate_self_signed_cert() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        let params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        let cert_der: CertificateDer<'static> = cert.into();
        let key_der = PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
        (cert_der, key_der)
    }

    #[test]
    fn tls_transport_describe_format() {
        let addr: SocketAddr = "1.2.3.4:443".parse().unwrap();
        let transport = TlsTransport::new(addr).with_server_name("example.com");
        assert_eq!(transport.describe(), "tls://1.2.3.4:443");
    }

    #[test]
    fn tls_transport_describe_with_ip() {
        let addr: SocketAddr = "1.2.3.4:443".parse().unwrap();
        let transport = TlsTransport::new(addr);
        assert_eq!(transport.describe(), "tls://1.2.3.4:443");
    }

    #[test]
    fn tls_transport_builder_methods() {
        let addr: SocketAddr = "1.2.3.4:443".parse().unwrap();
        let transport = TlsTransport::new(addr)
            .with_server_name("wraith.test")
            .with_insecure(true);
        assert_eq!(transport.tls_server_name, Some("wraith.test".to_string()));
        assert!(transport.insecure);
    }

    #[tokio::test]
    async fn tls_connect_insecure_self_signed() {
        let (cert_der, key_der) = generate_self_signed_cert();

        let acceptor = TlsAcceptor::bind(
            "127.0.0.1:0".parse().unwrap(),
            vec![cert_der],
            key_der,
            None,
        )
        .await
        .unwrap();
        let addr = acceptor.listen_addr();

        let transport = TlsTransport::new(addr)
            .with_server_name("localhost")
            .with_insecure(true);

        let accept_handle = tokio::spawn(async move { acceptor.accept().await.unwrap() });

        let mut client = transport.connect().await.unwrap();

        let (mut server, info) = accept_handle.await.unwrap();
        assert!(info.remote_addr.is_some());
        assert!(matches!(
            info.transport_kind,
            TransportKind::Tls { .. }
        ));

        client.write_all(b"hello tls").await.unwrap();
        let mut buf = [0u8; 9];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello tls");

        server.write_all(b"reply").await.unwrap();
        let mut buf = [0u8; 5];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"reply");
    }

    #[tokio::test]
    async fn tls_acceptor_returns_server_name() {
        let (cert_der, key_der) = generate_self_signed_cert();

        let acceptor = TlsAcceptor::bind(
            "127.0.0.1:0".parse().unwrap(),
            vec![cert_der],
            key_der,
            None,
        )
        .await
        .unwrap();
        let addr = acceptor.listen_addr();

        let transport = TlsTransport::new(addr)
            .with_server_name("localhost")
            .with_insecure(true);

        let accept_handle = tokio::spawn(async move { acceptor.accept().await.unwrap() });

        let _client = transport.connect().await.unwrap();

        let (_, info) = accept_handle.await.unwrap();
        if let TransportKind::Tls { server_name } = info.transport_kind {
            assert_eq!(server_name, Some("localhost".to_string()));
        } else {
            panic!("expected TransportKind::Tls");
        }
    }

    #[tokio::test]
    async fn tls_full_client_to_server_connection() {
        let (cert_der, key_der) = generate_self_signed_cert();

        let acceptor = TlsAcceptor::bind(
            "127.0.0.1:0".parse().unwrap(),
            vec![cert_der],
            key_der,
            None,
        )
        .await
        .unwrap();
        let addr = acceptor.listen_addr();

        let transport = TlsTransport::new(addr)
            .with_server_name("localhost")
            .with_insecure(true);

        let accept_handle = tokio::spawn(async move { acceptor.accept().await.unwrap() });

        let mut client = transport.connect().await.unwrap();
        let (mut server, _info) = accept_handle.await.unwrap();

        let msg = b"wraith integration test";
        client.write_all(msg).await.unwrap();
        let mut buf = vec![0u8; msg.len()];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf[..], msg);

        let reply = b"ok";
        server.write_all(reply).await.unwrap();
        let mut buf = [0u8; 2];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, reply);
    }

    #[tokio::test]
    async fn tls_acceptor_bind_port_zero_assigns_ephemeral() {
        let (cert_der, key_der) = generate_self_signed_cert();

        let acceptor = TlsAcceptor::bind(
            "127.0.0.1:0".parse().unwrap(),
            vec![cert_der],
            key_der,
            None,
        )
        .await
        .unwrap();
        assert_ne!(acceptor.listen_addr().port(), 0);
    }

    #[test]
    fn no_verifier_accepts_any_cert() {
        let verifier = NoVerifier;
        assert!(verifier.supported_verify_schemes().len() > 0);
    }
}