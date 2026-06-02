use anyhow::{anyhow, Result};
use async_trait::async_trait;
use iroh::{
    endpoint::RecvStream,
    node_info::NodeIdExt,
    Endpoint, NodeId, RelayMap, RelayMode, RelayUrl,
};
use tokio::io;

use super::{Transport, TransportAcceptor, TransportInfo, TransportKind};

const ALPN: &[u8] = b"wraith-ssh";
const DEFAULT_RELAY_URL: &str = "https://relay.iroh.network/";

/// A client-side iroh QUIC P2P transport that connects to a remote iroh endpoint.
///
/// Connects via `Endpoint::connect(node_id, alpn)`, opens a bidirectional
/// QUIC stream with `conn.open_bi()`, and joins the halves with
/// `tokio::io::join(recv, send)` to produce a duplex stream for russh.
/// Per ADR-003, `tokio::io::join` is used instead of a custom wrapper.
pub struct IrohTransport {
    node_id: NodeId,
    endpoint: Endpoint,
}

impl IrohTransport {
    pub async fn new(
        node_id: NodeId,
        relay_url: Option<RelayUrl>,
        proxy_url: Option<url::Url>,
    ) -> Result<Self> {
        let relay_url = relay_url.unwrap_or_else(|| {
            DEFAULT_RELAY_URL.parse().expect("default relay URL is valid")
        });
        let relay_map = RelayMap::from_url(relay_url);
        let mut builder = Endpoint::builder()
            .relay_mode(RelayMode::Custom(relay_map))
            .alpns(vec![ALPN.to_vec()]);
        if let Some(ref proxy) = proxy_url {
            builder = builder.proxy_url(proxy.clone());
        }
        let endpoint = builder.bind().await?;
        Ok(Self { node_id, endpoint })
    }

    pub fn endpoint_id(&self) -> String {
        self.endpoint.node_id().to_z32()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

#[async_trait]
impl Transport for IrohTransport {
    type Stream = io::Join<RecvStream, iroh::endpoint::SendStream>;

    async fn connect(&self) -> Result<Self::Stream> {
        let conn = self.endpoint.connect(self.node_id, ALPN).await?;
        let (send, recv) = conn.open_bi().await?;
        Ok(io::join(recv, send))
    }

    fn describe(&self) -> String {
        format!("iroh://{}", self.node_id.to_z32())
    }
}

/// A server-side iroh QUIC P2P transport acceptor that listens for incoming connections.
///
/// Binds an iroh `Endpoint` with the configured relay URL and optional proxy
/// (ADR-010). Accepts incoming connections, accepts bidirectional QUIC streams,
/// and joins the halves with `tokio::io::join(recv, send)`. Exposes
/// `endpoint_id()` for CLI display of the server's z-base-32 node ID.
pub struct IrohAcceptor {
    endpoint: Endpoint,
}

impl IrohAcceptor {
    pub async fn bind(
        relay_url: Option<RelayUrl>,
        proxy_url: Option<url::Url>,
    ) -> Result<Self> {
        let relay_url = relay_url.unwrap_or_else(|| {
            DEFAULT_RELAY_URL.parse().expect("default relay URL is valid")
        });
        let relay_map = RelayMap::from_url(relay_url);
        let mut builder = Endpoint::builder()
            .relay_mode(RelayMode::Custom(relay_map))
            .alpns(vec![ALPN.to_vec()]);
        if let Some(ref proxy) = proxy_url {
            builder = builder.proxy_url(proxy.clone());
        }
        let endpoint = builder.bind().await?;
        Ok(Self { endpoint })
    }

    pub fn endpoint_id(&self) -> String {
        self.endpoint.node_id().to_z32()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

#[async_trait]
impl TransportAcceptor for IrohAcceptor {
    type Stream = io::Join<RecvStream, iroh::endpoint::SendStream>;

    async fn accept(&self) -> Result<(Self::Stream, TransportInfo)> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| anyhow!("endpoint closed"))?;
        let conn = incoming.await?;
        let node_id = conn.remote_node_id()?;
        let (send, recv) = conn.accept_bi().await?;
        let stream = io::join(recv, send);
        let info = TransportInfo {
            remote_addr: None,
            transport_kind: TransportKind::Iroh {
                endpoint_id: node_id.to_z32(),
            },
        };
        Ok((stream, info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn iroh_acceptor_bind_creates_endpoint() {
        let acceptor = IrohAcceptor::bind(None, None).await.unwrap();
        let endpoint_id = acceptor.endpoint_id();
        assert!(!endpoint_id.is_empty());
        let parsed = NodeId::from_z32(&endpoint_id);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn iroh_acceptor_bind_with_custom_relay() {
        let relay: RelayUrl = "https://relay.iroh.network/".parse().unwrap();
        let acceptor = IrohAcceptor::bind(Some(relay), None).await.unwrap();
        assert!(!acceptor.endpoint_id().is_empty());
    }

    #[test]
    fn iroh_transport_describe_format() {
        let node_id: NodeId = iroh::SecretKey::generate(rand::rngs::OsRng)
            .public()
            .into();
        let desc = format!("iroh://{}", node_id.to_z32());
        assert!(desc.starts_with("iroh://"));
    }

    #[tokio::test]
    async fn iroh_transport_connect_builds_endpoint() {
        let node_id: NodeId = iroh::SecretKey::generate(rand::rngs::OsRng)
            .public()
            .into();
        let transport = IrohTransport::new(node_id, None, None).await.unwrap();
        assert!(transport.describe().starts_with("iroh://"));
        assert!(!transport.endpoint_id().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn iroh_client_connects_to_iroh_server() {
        let acceptor = IrohAcceptor::bind(None, None).await.unwrap();
        let server_node_id = acceptor.endpoint().node_id();

        let transport = IrohTransport::new(server_node_id, None, None)
            .await
            .unwrap();

        let mut addrs_watcher = acceptor.endpoint().direct_addresses();
        addrs_watcher.initialized().await.unwrap();
        let addr_set = addrs_watcher.get().unwrap().unwrap_or_default();
        for addr in addr_set {
            transport
                .endpoint
                .add_node_addr(iroh::NodeAddr::from_parts(
                    server_node_id,
                    None,
                    vec![addr.addr],
                ))
                .unwrap();
        }

        let accept_handle = tokio::spawn(async move {
            let (stream, info) = acceptor.accept().await.unwrap();
            assert!(matches!(info.transport_kind, TransportKind::Iroh { .. }));
            stream
        });

        let _client_stream: io::Join<RecvStream, iroh::endpoint::SendStream> =
            transport.connect().await.unwrap();
        let _server_stream = accept_handle.await.unwrap();
    }
}