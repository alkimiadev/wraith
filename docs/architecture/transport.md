---
status: draft
last_updated: 2026-06-01
---

# Transport Layer

## What

The transport layer produces a duplex byte stream (`AsyncRead + AsyncWrite + Unpin + Send`) that the SSH layer consumes via `russh::client::connect_stream()` or `russh::server::run_stream()`. The SSH layer is completely unaware of what transport it runs over.

## Why

Pluggable transports are the core architectural insight. They enable:

- **Simple deployment**: TCP on port 22 for basic use
- **Censorship resistance**: TLS on port 443 looks like HTTPS
- **NAT traversal**: iroh QUIC allows connections without public IPs
- **Composability**: transports can be layered (iroh through SOCKS5 through SSH through TLS)

Without this abstraction, each transport mode would need its own SSH connection logic. With it, there's one SSH implementation and N transport implementations.

## Architecture

### Transport Trait

```rust
// The core abstraction. Each transport produces ONE duplex stream.
// The SSH session runs over this stream for its entire lifetime.

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// Connect to the remote endpoint and return a duplex stream.
    /// For client-side transports.
    async fn connect(&self) -> Result<Self::Stream>;

    /// Return a human-readable description of this transport for logging.
    fn describe(&self) -> String;
}
```

### Server-Side Transport Acceptor

```rust
#[async_trait]
pub trait TransportAcceptor: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// Accept an incoming connection and return a duplex stream.
    async fn accept(&self) -> Result<(Self::Stream, TransportInfo)>;
}

/// Metadata about the incoming connection.
pub struct TransportInfo {
    pub remote_addr: Option<SocketAddr>,
    pub transport_kind: TransportKind,
}

pub enum TransportKind {
    Tcp,
    Tls { server_name: Option<String> },
    Iroh { peer_id: String },
}
```

### Transport Implementations

| Transport | Client | Server | Stream Type |
|-----------|--------|--------|-------------|
| **TcpTransport** | `TcpStream::connect(addr)` | `TcpListener::accept()` | `TcpStream` |
| **TlsTransport** | `TlsStream<TcpStream>` (client TLS) | `TlsStream<TcpStream>` (server TLS) | `tokio_rustls::client::TlsStream<TcpStream>` |
| **IrohTransport** | `endpoint.connect(peer, alpn)` then `conn.open_bi()` then `join(recv, send)` | `endpoint.accept()` then `conn.accept_bi()` then `join(recv, send)` | `tokio::io::Join<RecvStream, SendStream>` |

### Iroh Stream Join

Since QUIC splits streams into separate `RecvStream` and `SendStream`, while russh expects a single duplex stream, we combine them:

```rust
// One line. This works because RecvStream: AsyncRead and SendStream: AsyncWrite.
let stream = tokio::io::join(recv_stream, send_stream);
```

See ADR-003 for the decision to use `tokio::io::join` over a custom wrapper.

### Connection Lifecycle

```
Client                                          Server
  │                                               │
  │  transport.connect()                          │  transport_acceptor.accept()
  │  ─────────────────────────────────────────────▶│
  │        (duplex byte stream established)        │
  │                                               │
  │  russh::client::connect_stream(config,        │  russh::server::run_stream(config,
  │      stream, handler)                          │      stream, handler)
  │                                               │
  │  ═══════ SSH session over stream ═════════════ │
  │  ═════════════════════════════════════════════ │
  │                                               │
  │  channel_open_direct_tcpip(host, port, ...)  │
  │  ─────────────────────────────────────────────▶│
  │                                               │
  │  ┌─────── TCP proxy ──────────────────┐       │
  │  │  SSH channel ←→ TcpStream::connect │       │
  │  └────────────────────────────────────┘       │
```

## Transport Chaining

Transports can be nested. A common pattern: iroh transport through an upstream SOCKS5 proxy (which itself tunnels through another wraith instance):

```
wraith connect --transport iroh --proxy socks5://127.0.0.1:1080
```

The iroh endpoint's outbound TCP connections go through the SOCKS5 proxy, which itself tunnels through another wraith instance's SSH channel. The SOCKS5 proxy is provided by the core SOCKS5 server — no special chaining code needed.

## Constraints

- SSH sees only the stream. It never opens its own TCP connections. (ADR-004)
- Each transport produces exactly one stream per SSH session. Multiple sessions need multiple `connect()` calls.
- The iroh transport reuses a single `Endpoint` across multiple sessions (one QUIC connection per peer, multiple `open_bi()` streams). The endpoint is created once and shared.
- TLS transport requires certificate configuration on the server side. The client can accept any certificate (self-signed) or verify against a CA.

## Open Questions

- **OQ-02**: iroh relay configuration defaults
- **OQ-05**: Whether to support transport chaining in the CLI (`--transport iroh --proxy socks5://...`) or keep it as a separate manual configuration

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [001](decisions/001-pluggable-transport.md) | Pluggable transport | Transport trait produces stream, SSH consumes it |
| [003](decisions/003-iroh-stream-join.md) | iroh stream join | `tokio::io::join` combines QUIC halves |
| [004](decisions/004-ssh-over-transport.md) | SSH over transport | SSH never touches TCP/iroh/TLS directly |