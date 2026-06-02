---
id: review/server-and-client
name: Review server and client implementation — full SSH tunnel functionality
status: pending
depends_on:
  - meta/server-layer
  - meta/client-layer
  - review/core-foundation
scope: broad
risk: low
impact: phase
level: review
---

## Description

Review the server and client implementations after the core foundation review. This is a critical checkpoint before the CLI and NAPI layers — the server and client must work correctly as a unit before wrapping them in CLI flags or NAPI bindings.

Verify end-to-end SSH tunnel flow: client connects → SOCKS5 proxy works → port forwards work → reconnection works → server handles channels → proxy modes work → stealth mode works.

## Acceptance Criteria

- [ ] Server accepts SSH connections over TCP, TLS, iroh (via integration tests)
- [ ] Client establishes SSH sessions and runs SOCKS5 proxy
- [ ] Channel proxy: direct TCP, SOCKS5 proxy, HTTP CONNECT proxy all work
- [ ] Stealth mode: non-SSH gets nginx 404, SSH connects normally
- [ ] Rate limiting: connection limits enforced, auth attempt limits enforced
- [ ] Logging: structured `tracing::info!` events match ADR-013 format
- [ ] No logging of tunnel destinations (ADR-006)
- [ ] Reconnection: transport failure → exponential backoff → reconnect → port forwards re-registered
- [ ] Reserved `wraith-` destinations routed to control channel, not TCP proxy
- [ ] Graceful shutdown works for both server and client
- [ ] All tests pass: `cargo test --workspace`
- [ ] `cargo clippy --workspace` passes

## References

- docs/architecture/server.md, docs/architecture/client.md

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion