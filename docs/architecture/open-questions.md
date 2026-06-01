---
status: draft
last_updated: 2026-06-01
---

# Open Questions

## Transport

### OQ-01: TLS certificate management strategy
- **Origin**: [server.md](server.md)
- **Status**: open
- **Priority**: medium
- **Details**: Should the server support ACME/Let's Encrypt auto-provisioning (like https_proxy does), or is manual cert management sufficient? Auto-provisioning is more user-friendly but adds complexity and a dependency on the ACME protocol. Self-signed certs with `--insecure` flag on the client side covers the simple case.
- **Cross-references**: Server spec, TlsTransport implementation

### OQ-02: iroh relay configuration defaults
- **Origin**: [transport.md](transport.md)
- **Status**: open
- **Priority**: low
- **Details**: Should the default iroh relay be n0's free servers, or should users be required to specify one? n0's relay is convenient for testing and quick start but creates a dependency. Self-hosted relay is better for production. Consider: default to n0, allow `--iroh-relay` override, and document self-hosting.
- **Cross-references**: Transport spec, iroh docs

### OQ-05: Transport chaining support in CLI
- **Origin**: [transport.md](transport.md)
- **Status**: open
- **Priority**: low
- **Details**: Should `--transport iroh --proxy socks5://...` be supported natively, or should chaining be a manual configuration thing? The iroh transport's `connect()` method would need to route its outbound through the proxy. This is possible (iroh's `Endpoint::builder` supports proxy configuration) but adds CLI complexity. Consider: defer to Phase 2.
- **Cross-references**: Transport spec

## Client

### OQ-06: SSH config file parsing
- **Origin**: [client.md](client.md)
- **Status**: open
- **Priority**: low
- **Details**: Should the client read `~/.ssh/config` for default host/key/port settings? russh-config crate exists and can parse this. Would reduce CLI verbosity for frequent connections. Consider: `--config` flag to read from a wraith-specific config instead, avoiding OpenSSH config parsing complexity.
- **Cross-references**: Client spec

## Server

### OQ-07: ACME/Let's Encrypt support
- **Origin**: [server.md](server.md)
- **Status**: open
- **Priority**: medium
- **Details**: Auto-provisioning TLS certs from Let's Encrypt would make TLS mode much easier to set up. But it requires port 80 or port 443 + TLS-ALPN-01 challenge support, and a persistent cert store. Consider: defer to Phase 2, document manual cert setup for MVP.
- **Cross-references**: Server spec, TlsTransport

### OQ-08: Connection limits and rate limiting
- **Origin**: [server.md](server.md)
- **Status**: open
- **Priority**: low
- **Details**: Should the server support configurable connection limits, rate limiting, and max simultaneous channels? Useful for preventing abuse on public-facing servers. Consider: `--max-connections` and `--max-channels-per-connection` flags.
- **Cross-references**: Server spec

### OQ-04: Authentication beyond Ed25519 keys
- **Origin**: [client.md](client.md), [server.md](server.md)
- **Status**: open
- **Priority**: low
- **Details**: Should password authentication be supported? Should SSH certificates (OpenSSH cert-authority) be supported? Password auth is convenient but less secure. Certificates are useful for large-scale deployments. Consider: password auth as optional flag (`--allow-password`), certificates as future feature.
- **Cross-references**: Client spec, Server spec

## TUN

### OQ-03: Windows TUN support scope
- **Origin**: [tun-shim.md](tun-shim.md)
- **Status**: open
- **Priority**: low
- **Details**: tun-rs supports Windows via wintun.dll but distributing a DLL adds complexity. Consider: Linux and macOS only for MVP, Windows as a follow-up.
- **Cross-references**: tun-shim.md

### OQ-09: TCP reconstruction approach for TUN
- **Origin**: [tun-shim.md](tun-shim.md)
- **Status**: open
- **Priority**: medium
- **Details**: Should the TUN shim use a userspace TCP stack (like smoltcp or tun2proxy's ip-stack) for reliable TCP reconstruction, or forward raw IP packets through SOCKS5? Raw packet forwarding requires handling segmentation, retransmission, and reordering. Userspace TCP solves this but is more code. Consider: start with SOCKS5 proxying (each TUN packet becomes a SOCKS5 connection) and add TCP reconstruction if needed.
- **Cross-references**: tun-shim.md

## NAPI / PubSub

### OQ-10: NAPI wrapper API surface
- **Origin**: [napi-and-pubsub.md](napi-and-pubsub.md)
- **Status**: open
- **Priority**: medium
- **Details**: Should the NAPI wrapper expose just a `connect()` function returning a `Duplex` stream, or also expose `serve()` for server-side use from Node.js? Server-side would enable running a wraith server from a Node.js process. Consider: `connect()` only for MVP, `serve()` as follow-up.
- **Cross-references**: napi-and-pubsub.md

### OQ-11: napi-rs vs uniffi for FFI bridge
- **Origin**: [napi-and-pubsub.md](napi-and-pubsub.md)
- **Status**: open
- **Priority**: low
- **Details**: napi-rs is the standard for Node.js native addons and has the best ecosystem. uniffi supports more targets (Python, Swift, Kotlin) but is less mature for Node.js. Since the primary consumer is TypeScript/Node.js (pubsub/operations ecosystem), napi-rs is the logical choice. But if future Python or mobile consumers are anticipated, uniffi could be worth the investment.
- **Cross-references**: napi-and-pubsub.md