---
status: draft
last_updated: 2026-06-01
---

# Server

## What

The wraith server accepts SSH connections (via pluggable transport) and handles `channel_open_direct_tcpip` requests by connecting to the requested target — either directly or through an outbound proxy.

## Why

The server is the tunnel endpoint. It receives SSH channels requesting TCP connections to specific hosts and ports, and makes those connections on behalf of the client. It's the same role as an SSH server with `AllowTcpForwarding yes`, but self-contained and transport-agnostic.

## Architecture

### Server Components

```
┌──────────────────────────────────────────────────┐
│                   wraith serve                     │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │          SSH Server (russh)                  │ │
│  │   ServerHandler per connection              │ │
│  │   - auth_publickey() → Accept/Reject        │ │
│  │   - channel_open_direct_tcpip() → connect   │ │
│  │   - channel_open_forwarded_tcpip() → proxy  │ │
│  └──────────────────┬──────────────────────────┘ │
│                      │                            │
│  ┌──────────────────▼──────────────────────────┐ │
│  │         Transport Acceptor                   │ │
│  │   (TcpListener / TlsListener / IrohEndpoint) │ │
│  └──────────────────────────────────────────────┘ │
│                                                   │
│  ┌──────────────────────────────────────────────┐ │
│  │         Outbound Proxy (optional)             │ │
│  │   - Direct TCP                               │ │
│  │   - SOCKS5 proxy                             │ │
│  │   - HTTP CONNECT proxy                       │ │
│  └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

### Authentication

The server supports Ed25519 public key authentication by default:

1. Load authorized keys from `~/.ssh/authorized_keys` or a specified path
2. `auth_publickey()` checks the presented key against the authorized set
3. Uses constant-time comparison to prevent timing attacks

Optional password authentication (not recommended, controlled by feature flag or CLI flag).

### Channel Handling

When a client opens a `channel_open_direct_tcpip(host, port, originator_addr, originator_port)`:

1. **ACL check** — verify the client is allowed to connect to `host:port` (if ACLs are configured)
2. **Outbound connection** — connect to the target, either directly or via the configured outbound proxy
3. **Bidirectional proxy** — `tokio::io::copy_bidirectional` between the SSH channel stream and the outbound TCP stream
4. **Cleanup** — close the channel and TCP stream when either side disconnects

### Outbound Proxy Modes

| Mode | CLI Flag | Behavior |
|------|----------|----------|
| **Direct** | (default) | `TcpStream::connect(target)` |
| **SOCKS5** | `--proxy socks5://addr:port` | Connect through SOCKS5 proxy |
| **HTTP CONNECT** | `--proxy http://addr:port` | Connect through HTTP CONNECT proxy |

The proxy setting applies globally to all outbound connections from the server.

### Stealth Mode

When `--stealth` is enabled on the server alongside TLS transport:

1. Non-SSH connections (normal web browsers, scanners) receive a fake nginx 404 response
2. The server detects whether the connecting client is speaking SSH or HTTP after the TLS handshake
3. If SSH: proceed with `server::run_stream()`
4. If HTTP: respond with `HTTP/1.1 404 Not Found` + `Server: nginx` headers, then close

This makes the server appear as an ordinary web server to port scanners and DPI systems.

### Server Handler (russh)

```rust
struct WraithServerHandler {
    authorized_keys: HashSet<PublicKey>,
    proxy_config: Option<ProxyConfig>,
}

impl server::Handler for WraithServerHandler {
    type Error = anyhow::Error;

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Auth {
        if self.authorized_keys.contains(key) {
            Auth::Accept
        } else {
            Auth::Reject { proceed_with_methods: None, partial_success: false }
        }
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        originator_addr: &str,
        originator_port: u32,
        session: &mut server::Session,
    ) -> Result<Channel<server::Msg>, Self::Error> {
        // ACL check (if configured)
        // Connect to host:port (directly or via proxy)
        // Spawn bidirectional proxy task
        Ok(channel)
    }
}
```

### Logging

- **Log**: Auth attempts (timestamp, source IP, user, key fingerprint, success/failure)
- **Do not log**: Channel open targets, DNS resolutions, bytes transferred, connection duration

This provides enough information for fail2ban integration without creating a privacy-sensitive audit trail.

### CLI Interface

```bash
# Basic server (SSH on port 22)
wraith serve --key ~/.ssh/ssh_host_ed25519_key

# With TLS on port 443
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport tls \
    --tls-cert /etc/ssl/cert.pem \
    --tls-key /etc/ssl/key.pem

# With TLS + stealth (fake nginx 404 to scanners)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport tls \
    --tls-cert /etc/ssl/cert.pem \
    --tls-key /etc/ssl/key.pem \
    --stealth

# With iroh transport (no public IP needed)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport iroh

# With outbound proxy
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --proxy socks5://127.0.0.1:9050

# All options
wraith serve \
  --key <path> \              # SSH host key path (required)
  --authorized-keys <path> \  # Authorized keys file (default: ~/.ssh/authorized_keys)
  --transport tcp|tls|iroh \  # Transport mode
  --listen <addr:port> \      # Listen address for TCP/TLS (default: 0.0.0.0:22)
  --tls-cert <path> \         # TLS certificate (required for tls transport)
  --tls-key <path> \          # TLS private key (required for tls transport)
  --stealth \                 # Serve fake nginx 404 to non-SSH connections
  --proxy <url> \             # Outbound proxy URL (socks5:// or http://)
  --iroh-relay <url>          # iroh relay server URL (default: n0 relay)
```

### iroh Server Mode

When running with `--transport iroh`, the server:

1. Creates an `iroh::Endpoint` with the SSH ALPN
2. Prints its `EndpointId` (Ed25519 public key) — this is what clients use to connect
3. Uses `iroh::protocol::Router` to accept incoming connections
4. For each connection, accepts a `open_bi()` stream and passes it to `server::run_stream()`

No listening port is needed. The server connects outbound to the iroh relay and awaits connections from clients who know its `EndpointId`.

## Constraints

- The server does not log tunnel destinations (ADR-006, pending)
- One `ServerHandler` instance per connection. Handler state is not shared between connections (unless explicitly configured via `Arc` shared state for things like connection limits).
- The server binds to a single transport at a time. Running multiple transports (e.g., TCP + iroh) simultaneously requires separate processes or a future multiplexing feature.

## Open Questions

- **OQ-07**: Whether to support ACME/Let's Encrypt auto-provisioning for TLS certificates
- **OQ-08**: Connection limits and rate limiting configuration

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [001](decisions/001-pluggable-transport.md) | Pluggable transport | Transport trait, SSH consumes stream |
| [004](decisions/004-ssh-over-transport.md) | SSH over transport | SSH never touches network directly |