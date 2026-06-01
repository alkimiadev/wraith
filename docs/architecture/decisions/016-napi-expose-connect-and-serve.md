# ADR-016: NAPI Exposes Both connect() and serve()

## Status
Accepted

## Context
The NAPI wrapper needs to provide TypeScript/Node.js consumers with access to wraith's functionality. The primary use case is `@alkdev/pubsub`'s event target system, which needs both directions:

1. **connect()**: Establish a client connection to a wraith server. Used by workers/spokes that need to tunnel events through a wraith server.
2. **serve()**: Start a wraith server from Node.js. Used by hubs that want to accept wraith connections and route events.

The previous decision (ADR-007) was to expose only `connect()` for MVP, deferring `serve()`. However, the pubsub integration requires both: a spoke needs `connect()` to reach a hub, and a hub could use `serve()` to accept connections without running a separate `wraith serve` process.

More importantly, both `connect()` and `serve()` are fundamental operations of the wraith library. Since the NAPI wrapper is a thin layer over `wraith-core`, exposing both is straightforward — they're just Rust functions behind `#[napi]` attributes.

## Decision
The NAPI wrapper exposes both `connect()` and `serve()` from the start:

```typescript
// @alkdev/wraith
function connect(options: WraithConnectOptions): Promise<Duplex>;
function serve(options: WraithServeOptions): Promise<WraithServer>;
```

- `connect()` returns a `Duplex` stream (as per ADR-007)
- `serve()` returns a `WraithServer` object with a `close()` method and events for new connections

The NAPI layer is transport-agnostic — it doesn't know about pubsub's `EventEnvelope`. The pubsub event target adapter wraps the `Duplex` stream to implement `TypedEventTarget`. This separation ensures the NAPI wrapper is reusable for any stream-based protocol, not just pubsub.

## Consequences
- **Positive**: Pubsub can use both directions without running a separate binary for the server side.
- **Positive**: The NAPI wrapper becomes a complete bridge — any Node.js process can be either a client or server.
- **Positive**: Implementation is still minimal — `serve()` is just `wraith_core::server::run()` behind `#[napi]`.
- **Negative**: Slightly larger API surface (two functions + `WraithServer` type instead of just `connect()`).
- **Negative**: Server-side NAPI needs to handle multiple concurrent connections, which adds complexity to `WraithServer`.

## References
- [napi-and-pubsub.md](../napi-and-pubsub.md)
- [ADR-007](007-napi-single-stream.md) — still valid; NAPI exposes single streams, but now from both sides
- [OQ-10](../open-questions.md) — resolved by this ADR