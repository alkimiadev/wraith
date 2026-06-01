---
status: draft
last_updated: 2026-06-01
---

# Wraith Overview

## Purpose

Wraith is a self-hostable SSH-based tunnel tool that provides VPN-like functionality without being a VPN protocol. It enables:

- **Private tunneling** of services (Postgres, Redis, internal APIs) over SSH
- **Censorship circumvention** — SSH over TLS on port 443 looks like HTTPS to DPI
- **NAT traversal** — iroh transport allows peer-to-peer connections without public IPs or port forwarding
- **Service mesh connectivity** — a lightweight transport layer for the pubsub/operations event system

The core insight: SSH tunnels work because SSH is fundamental infrastructure. Blocking it breaks the internet. Wraith makes SSH tunneling accessible through a simple CLI with pluggable transports.

## Exports

### Binary: `wraith`

A single binary with subcommands:

```
wraith serve     — Start the server (accepts SSH connections)
wraith connect  — Start the client (opens SSH session, exposes SOCKS5/port-forwards)
```

### Library: `wraith-core`

The `wraith-core` crate exports the pluggable components for embedding or programmatic use:

- `Transport` trait — produces a duplex stream for SSH to run over
- `TcpTransport` — direct TCP connection
- `TlsTransport` — TCP + tokio-rustls TLS
- `IrohTransport` — iroh QUIC P2P connection
- `Socks5Server` — local SOCKS5 proxy that forwards through SSH channels
- `PortForwarder` — manages local/remote port forwards
- `ServerHandler` — russh server handler with configurable auth and channel policies

## Dependencies

| Dependency | Purpose | Feature-gated |
|------------|---------|---------------|
| `russh` | SSH client & server | No (core) |
| `tokio` | Async runtime | No (core) |
| `tokio-rustls` | TLS wrapping | Yes (`tls`) |
| `rustls` | TLS implementation | Yes (`tls`) |
| `iroh` | P2P QUIC transport | Yes (`iroh`) |
| `tun-rs` | TUN interface | Yes (`tun`) |
| `clap` | CLI argument parsing | No (core) |
| `tracing` | Structured logging | No (core) |
| `anyhow` / `thiserror` | Error handling | No (core) |

## Architecture Constraints

1. **SSH runs over transport, not alongside** — The transport layer produces a single `AsyncRead+AsyncWrite+Unpin+Send` stream. SSH runs over that stream via `russh::client::connect_stream()` / `russh::server::run_stream()`. The SSH layer never knows what transport it's on. (ADR-001, ADR-004)

2. **TUN is a separate process** — The core binary never requires root. TUN functionality is a separate `wraith-tun` process that reads from a TUN device and forwards to the core's SOCKS5 port. (ADR-002)

3. **SOCKS5 is the primary client interface** — Port forwarding and TUN are built on top of SOCKS5, not alongside it. Everything flows through the SSH channel abstraction. (ADR-005)

4. **No logging of tunnel destinations** — The server logs auth attempts (for fail2ban) but does not log `channel_open_direct_tcpip` destinations, DNS lookups, or bytes transferred. (ADR-006, pending)

5. **Feature flags control transport inclusion** — `tls`, `iroh`, `tun` are feature-gated so the base install is lean. Users opt in to heavier dependencies.

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [001](decisions/001-pluggable-transport.md) | Pluggable transport | Transport trait produces `AsyncRead+AsyncWrite+Unpin+Send`, SSH consumes it |
| [002](decisions/002-tun-separate-process.md) | TUN shim separate | Core binary unprivileged, TUN is a thin root wrapper |
| [003](decisions/003-iroh-stream-join.md) | iroh stream join | `tokio::io::join(recv, send)` combines QUIC halves |
| [004](decisions/004-ssh-over-transport.md) | SSH over transport | SSH never accesses TCP/iroh/TLS directly |
| [005](decisions/005-socks5-before-tun.md) | SOCKS5 first | SOCKS5 is the primary interface, TUN forwards to it |

## Open Questions

- **OQ-01**: TLS certificate management strategy (ADR-006, pending)
- **OQ-02**: iroh relay configuration defaults (n0 relay vs self-hosted)
- **OQ-03**: Windows TUN support scope (wintun.dll dependency)
- **OQ-04**: Authentication beyond Ed25519 keys (password auth, certificate auth)

## References

- [Feasibility Assessment](../../../conversations/research/ssh-tunnel-vpn-alternative-feasibility.md)
- [russh API](/workspace/russh) — SSH client/server library
- [Dispatch](/workspace/@alkdev/dispatch) — Reference implementation of russh port forwarding
- [tun-rs](https://github.com/tun-rs/tun-rs) — Cross-platform TUN/TAP library
- [iroh](/workspace/iroh) — P2P QUIC connections