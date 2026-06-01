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
│  │   ServerHandler per connection               │ │
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
│  │         Outbound Proxy (optional)            │ │
│  │   - Direct TCP                               │ │
│  │   - SOCKS5 proxy                            │ │
│  │   - HTTP CONNECT proxy                      │ │
│  └──────────────────────────────────────────────┘ │
│                                                   │
│  ┌──────────────────────────────────────────────┐ │
│  │         Rate Limiter                         │ │
│  │   - max-connections-per-ip                   │ │
│  │   - max-auth-attempts                        │ │
│  └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

### Authentication

The server supports Ed25519 public key authentication (default) and OpenSSH certificate authority authentication (ADR-012):

**Ed25519 public key** (default):
1. Load authorized keys from a specified path or in-memory data
2. `auth_publickey()` checks the presented key against the authorized set
3. Uses constant-time comparison to prevent timing attacks

**OpenSSH certificate authority** (ADR-012):
1. Load a trusted CA public key (`--cert-authority <path>`)
2. `auth_publickey()` validates the presented certificate: checks CA signature, expiry, and principal restrictions
3. Supports certificate options: `permit-port-forwarding`, `no-pty`, `source-address`

This enables multi-user deployments where adding one CA line to `authorized_keys` is simpler than managing individual keys for every user.

**No password authentication over SSH.** Keys and certificates are sufficient and more secure. If a local SOCKS5 proxy needs its own auth layer, that's a separate concern.

### TLS Certificate Provisioning

The server supports three TLS certificate modes (ADR-008):

1. **Manual certs** (`--tls-cert` / `--tls-key`): User provides certificate and key files. For users with existing PKI.
2. **Domain-based ACME** (`--acme-domain <domain>`): Auto-provisions certificates from Let's Encrypt using HTTP-01 or TLS-ALPN-01 challenges. Certificate is domain-bound and auto-renews. Requires port 80 or DNS access for challenges.
3. **IP-based ACME**: Short-lived certificates via TLS-ALPN-01 challenge on port 443. No domain name needed, but certificates expire frequently. The ACME client runs continuously.

ACME support is feature-gated behind the `acme` feature flag to keep the base binary lean. Implementation uses `rustls-acme` or a similar pure-Rust ACME client to avoid an external `certbot` dependency.

### Channel Handling

When a client opens a `channel_open_direct_tcpip(host, port, originator_addr, originator_port)`:

1. **Connection** — connect to `host:port`, either directly or via the configured outbound proxy
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
    cert_authorities: Vec<PublicKey>,
    proxy_config: Option<ProxyConfig>,
}

impl server::Handler for WraithServerHandler {
    type Error = anyhow::Error;

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Auth {
        // Check direct key match
        if self.authorized_keys.contains(key) {
            return Auth::Accept;
        }
        // Check certificate authority validation
        if let Some(cert) = key.as_certificate() {
            for ca in &self.cert_authorities {
                if cert.verify(ca) && cert.is_valid() {
                    return Auth::Accept;
                }
            }
        }
        Auth::Reject { proceed_with_methods: None, partial_success: false }
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
        // Connect to host:port (directly or via proxy)
        // Spawn bidirectional proxy task
        Ok(channel)
    }
}
```

### Logging and Rate Limiting

**Logging** (for fail2ban integration on Linux):

- `INFO` level: auth attempts (remote_addr, user, key_fingerprint, accept/reject)
- `INFO` level: connection opened (remote_addr, transport kind)
- `INFO` level: connection closed (remote_addr, duration)
- Do NOT log: channel open targets, DNS resolutions, bytes transferred

This matches our production fail2ban setup which filters on source IP + failure indicators. Example log lines:
```
INFO auth attempt remote_addr=203.0.113.50 user=root key_fingerprint=SHA256:abc... result=reject
INFO connection opened remote_addr=203.0.113.50 transport=tls
```

**Built-in rate limiting** (platform-independent):

| Flag | Default | Purpose |
|------|---------|---------|
| `--max-connections-per-ip` | 0 (unlimited) | Reject new connections from IPs with N active connections |
| `--max-auth-attempts` | 10 | Disconnect after N failed auth attempts per connection |

These provide abuse protection on platforms without fail2ban (macOS, Windows, BSD) and complement fail2ban on Linux.

### CLI Interface

```bash
# Basic server (SSH on port 22)
wraith serve --key ~/.ssh/ssh_host_ed25519_key

# With TLS (manual certs)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport tls \
    --tls-cert /etc/ssl/cert.pem \
    --tls-key /etc/ssl/key.pem

# With TLS (auto ACME, domain-based)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport tls \
    --acme-domain example.com

# With TLS + stealth (fake nginx 404 to scanners)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport tls \
    --acme-domain example.com \
    --stealth

# With iroh transport (no public IP needed)
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --transport iroh

# With outbound proxy
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --proxy socks5://127.0.0.1:9050

# With certificate authority authentication
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --cert-authority /etc/wraith/ca.pub

# With rate limiting
wraith serve --key ~/.ssh/ssh_host_ed25519_key \
    --max-connections-per-ip 5 \
    --max-auth-attempts 3

# All options
wraith serve \
  --key <path-or-buffer> \       # SSH host key (required)
  --authorized-keys <path> \     # Authorized keys file
  --cert-authority <path> \      # CA public key for cert-auth
  --transport tcp|tls|iroh \     # Transport mode
  --listen <addr:port> \         # Listen address for TCP/TLS (default: 0.0.0.0:22)
  --tls-cert <path> \            # TLS certificate (manual)
  --tls-key <path> \            # TLS private key (manual)
  --acme-domain <domain> \      # ACME auto-cert domain
  --stealth \                    # Serve fake nginx 404 to non-SSH connections
  --proxy <url> \                # Outbound proxy URL (socks5:// or http://)
  --iroh-relay <url> \           # iroh relay server URL (default: n0 relay)
  --max-connections-per-ip <n> \ # Max concurrent connections per IP (default: unlimited)
  --max-auth-attempts <n>        # Max auth failures before disconnect (default: 10)
```

### iroh Server Mode

When running with `--transport iroh`, the server:

1. Creates an `iroh::Endpoint` with ALPN value `b"wraith-ssh"`
2. Prints its `EndpointId` (Ed25519 public key) — this is what clients use to connect
3. Uses `iroh::protocol::Router` to accept incoming connections
4. For each connection, accepts a `open_bi()` stream and passes it to `server::run_stream()`

No listening port is needed. The server connects outbound to the iroh relay (default: n0, override with `--iroh-relay`) and awaits connections from clients who know its `EndpointId`.

## Constraints

- The server does not log tunnel destinations (ADR-006). Auth events and connection events are logged for fail2ban integration (ADR-013).
- One `ServerHandler` instance per connection. Handler state is not shared between connections (unless explicitly configured via `Arc` shared state for things like connection limits).
- The server binds to a single transport at a time. Running multiple transports (e.g., TCP + iroh) simultaneously requires separate processes or a future multiplexing feature.
- ACME support requires the `acme` feature flag. Without it, only manual TLS certs are supported.
- No password authentication over SSH channels. Key-based and cert-authority only (ADR-012).

## Open Questions

None — all resolved.

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [001](decisions/001-pluggable-transport.md) | Pluggable transport | Transport trait, SSH consumes stream |
| [004](decisions/004-ssh-over-transport.md) | SSH over transport | SSH never touches network directly |
| [006](decisions/006-no-logging-of-tunnel-destinations.md) | No logging of destinations | Server logs auth and connections, not destinations |
| [008](decisions/008-acme-lets-encrypt.md) | ACME/Let's Encrypt | Auto-provision TLS certs, domain and IP paths |
| [012](decisions/012-auth-ed25519-and-cert-authority.md) | Key + cert-authority auth | No password auth; support OpenSSH cert-authority |
| [013](decisions/013-fail2ban-friendly-logging.md) | Fail2ban-friendly logging | Structured auth logs + built-in rate limiting |
| [017](decisions/017-stealth-mode-protocol-multiplexing.md) | Stealth mode | Protocol multiplexing on port 443 |
| [018](decisions/018-control-channel-for-pubsub.md) | Control channel | Reserved `wraith-control` destination for pubsub |