---
id: napi/serve-function
name: Implement NAPI serve() — server with connection events returning Duplex streams
status: pending
depends_on:
  - napi/project-setup
  - server/serve-loop
scope: moderate
risk: high
impact: component
level: implementation
---

## Description

Implement the NAPI `serve()` function per ADR-016. Returns a `WraithServer` object with a `close()` method and `onConnection` event emitter. Each incoming SSH connection produces a `Duplex` stream.

The function accepts `WraithServeOptions` and returns `Promise<WraithServer>`. The NAPI layer handles transport binding, SSH server setup, and connection handling.

## Acceptance Criteria

- [ ] `#[napi]` function `serve(options: WraithServeOptions) -> Result<WraithServer>` in `crates/wraith-napi/src/serve.rs`
- [ ] `WraithServeOptions` struct with napi fields: `transport`, `hostKey`, `authorizedKeys`, `certAuthority`, `tlsCert`, `tlsKey`, `acmeDomain`, `listen`, `irohRelay`
- [ ] `WraithServer` napi class with `close() -> Promise<void>` and `onConnection(callback)` event registration
- [ ] Each incoming connection produces a `Duplex` stream via the `onConnection` callback
- [ ] `ConnectionInfo` struct passed with each connection: `remoteAddr`, `transportKind`
- [ ] Key material: `hostKey`, `authorizedKeys` accept file path (string) or `Buffer` (in-memory)
- [ ] Server starts transport acceptor, authenticates connections, emits stream events
- [ ] `close()` triggers graceful shutdown
- [ ] TypeScript type matches napi-and-pubsub.md spec
- [ ] Integration test: JS serve() + connect() round-trip works

## References

- docs/architecture/napi-and-pubsub.md — NAPI serve() spec, WraithServer interface
- docs/architecture/decisions/016-napi-expose-connect-and-serve.md — both connect() and serve()
- docs/architecture/server.md — server configuration

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion