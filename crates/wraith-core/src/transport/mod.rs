mod tcp;

pub use tcp::{TcpAcceptor, TcpTransport};

use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    async fn connect(&self) -> Result<Self::Stream>;

    fn describe(&self) -> String;
}

#[async_trait]
pub trait TransportAcceptor: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    async fn accept(&self) -> Result<(Self::Stream, TransportInfo)>;
}

/// Metadata about an incoming transport connection.
///
/// Carries the remote address (if available) and the kind of transport
/// used. The server handler uses this for logging and auth decisions.
/// See ADR-001 for the pluggable transport rationale and ADR-004
/// for why SSH runs entirely over the transport stream.
#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub remote_addr: Option<SocketAddr>,
    pub transport_kind: TransportKind,
}

/// The kind of transport that produced a connection.
///
/// Each variant identifies the transport mechanism. Used by the
/// server handler for logging and authorization decisions.
/// See ADR-001 and ADR-004.
#[derive(Debug, Clone)]
pub enum TransportKind {
    Tcp,
    Tls {
        server_name: Option<String>,
    },
    Iroh {
        endpoint_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, DuplexStream};

    struct MockTransport;

    #[async_trait]
    impl Transport for MockTransport {
        type Stream = DuplexStream;

        async fn connect(&self) -> Result<Self::Stream> {
            let (stream, _) = duplex(1024);
            Ok(stream)
        }

        fn describe(&self) -> String {
            "mock".to_string()
        }
    }

    struct MockAcceptor;

    #[async_trait]
    impl TransportAcceptor for MockAcceptor {
        type Stream = DuplexStream;

        async fn accept(&self) -> Result<(Self::Stream, TransportInfo)> {
            let (stream, _) = duplex(1024);
            let info = TransportInfo {
                remote_addr: None,
                transport_kind: TransportKind::Tcp,
            };
            Ok((stream, info))
        }
    }

    #[tokio::test]
    async fn transport_trait_object() {
        let _boxed: Box<dyn Transport<Stream = DuplexStream>> = Box::new(MockTransport);
    }

    #[tokio::test]
    async fn transport_acceptor_trait_object() {
        let _boxed: Box<dyn TransportAcceptor<Stream = DuplexStream>> = Box::new(MockAcceptor);
    }

    #[tokio::test]
    async fn transport_connect_returns_stream() {
        let t = MockTransport;
        let _stream = t.connect().await.unwrap();
    }

    #[tokio::test]
    async fn transport_describe_returns_string() {
        let t = MockTransport;
        assert_eq!(t.describe(), "mock");
    }

    #[tokio::test]
    async fn acceptor_accept_returns_stream_and_info() {
        let a = MockAcceptor;
        let (_, info) = a.accept().await.unwrap();
        assert!(info.remote_addr.is_none());
        assert!(matches!(info.transport_kind, TransportKind::Tcp));
    }

    #[test]
    fn transport_kind_variants() {
        let tcp = TransportKind::Tcp;
        let tls = TransportKind::Tls {
            server_name: Some("example.com".to_string()),
        };
        let iroh = TransportKind::Iroh {
            endpoint_id: "abc123".to_string(),
        };

        if let TransportKind::Tcp = tcp {}
        if let TransportKind::Tls {
            server_name: Some(name),
        } = tls
        {
            assert_eq!(name, "example.com");
        }
        if let TransportKind::Iroh { endpoint_id } = iroh {
            assert_eq!(endpoint_id, "abc123");
        }
    }
}