---
id: server/serve-loop
name: Implement server accept loop, graceful shutdown, and ServeOptions config
status: pending
depends_on:
  - server/handler
  - server/channel-proxy
  - server/rate-limiting-and-logging
  - transport/trait-and-types
scope: moderate
risk: medium
impact: component
level: implementation
---

## Description

Implement the server's main accept loop and configuration. This ties together the transport acceptor, server handler, rate limiting, and logging into a coherent server process.

`ServeOptions` is the programmatic configuration struct (ADR-011) for the server. The accept loop:
1. Binds a `TransportAcceptor` based on transport mode
2. Accepts incoming connections (respecting rate limits)
3. Creates a `ServerHandler` per connection
4. Passes the stream to `russh::server::run_stream()`
5. Handles graceful shutdown on SIGTERM/SIGINT

## Acceptance Criteria

- [ ] `crates/wraith-core/src/server/mod.rs` re-exports all server components
- [ ] `ServeOptions` struct with fields matching server.md CLI interface: `key`, `authorized_keys`, `cert_authority`, `transport_mode`, `listen_addr`, `tls_cert`, `tls_key`, `acme_domain`, `stealth`, `proxy`, `iroh_relay`, `max_connections_per_ip`, `max_auth_attempts`
- [ ] `Server::new(opts: ServeOptions) -> Result<Server>` — creates server with bound acceptor, auth config, rate limiter
- [ ] `Server::run()` — enters accept loop, for each connection: check rate limit → create handler → `run_stream()`
- [ ] Stealth mode integration: if enabled, protocol detection before `run_stream()`
- [ ] Graceful shutdown: `Server::shutdown()` method and signal handler (SIGTERM/SIGINT)
  - Stop accepting new connections
  - Send SSH disconnect to active sessions
  - Wait for drain timeout (~2 seconds per session)
  - Forcibly terminate remaining connections
- [ ] iroh mode: prints endpoint ID on startup
- [ ] `ServeOptions::key` and `ServeOptions::authorized_keys` accept `KeySource` (file or in-memory)
- [ ] Integration test: start server, client connects via mock transport, session works, shutdown completes

## References

- docs/architecture/server.md — full server spec including graceful shutdown
- docs/architecture/decisions/011-no-ssh-config-programmatic-api.md — ServeOptions programmatic struct

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion