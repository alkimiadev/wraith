---
id: review/complete-system
name: Review complete system — CLI, NAPI, end-to-end integration
status: pending
depends_on:
  - meta/cli-layer
  - meta/napi-layer
  - review/server-and-client
scope: system
risk: low
impact: project
level: review
---

## Description

Final review of the complete wraith system. Verify CLI binary works end-to-end, NAPI wrapper provides correct JavaScript API, and both layers properly wrap the core library.

## Acceptance Criteria

- [ ] `wraith serve` + `wraith connect` end-to-end: SSH tunnel established, SOCKS5 proxy routes traffic
- [ ] All CLI flags work: transport modes (tcp, tls, iroh), auth options, proxy, stealth, rate limits
- [ ] Environment variables (`WRAITH_SERVER`, `WRAITH_IDENTITY`) work as defaults
- [ ] `--stealth` validates `--transport tls` requirement
- [ ] NAPI `connect()` returns Duplex stream; data flows bidirectionally
- [ ] NAPI `serve()` accepts connections; `onConnection` emits Duplex streams
- [ ] NAPI key material from Buffer works (not just file paths)
- [ ] Feature flags: `tls`, `iroh`, `acme` correctly gate optional functionality
- [ ] Base build (`cargo build -p wraith-core` with no features) compiles and works
- [ ] All tests pass: `cargo test --workspace`
- [ ] NAPI tests pass: `cd crates/wraith-napi && npm test`
- [ ] `cargo clippy --workspace` passes
- [ ] No logging of tunnel destinations anywhere in the system

## References

- docs/architecture/overview.md, docs/architecture/napi-and-pubsub.md

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion