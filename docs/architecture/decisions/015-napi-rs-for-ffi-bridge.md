# ADR-015: napi-rs for FFI Bridge

## Status
Accepted

## Context
The NAPI wrapper needs a Rust-to-Node.js bridge. Two main options:

1. **napi-rs**: The standard for Rust → Node.js native addons. Mature, well-documented, large ecosystem. Produces `.node` binaries for specific platforms. Good build tooling (`napi` CLI). Used by major projects (swc, rspack, biome).

2. **uniffi**: Mozilla's FFI bridge supporting multiple targets (Python, Swift, Kotlin, Node.js). Broader target reach but less mature for Node.js specifically. The Node.js binding is relatively new.

The primary consumer is TypeScript/Node.js — specifically the `@alkdev/pubsub` event target system. The broader alkdev ecosystem (pubsub, operations) is TypeScript-first. While future Python or mobile consumers are imaginable, they are not in scope.

## Decision
Use napi-rs. It's the standard for Node.js native addons, has the best documentation and tooling, and matches our primary consumer (TypeScript/Node.js). If future Python or mobile consumers are needed, uniffi can be added as a separate FFI layer — the Rust core library doesn't change, only the binding layer does.

## Consequences
- **Positive**: Best-in-class Node.js native addon support. Mature, well-documented, widely used.
- **Positive**: `napi` CLI handles building, cross-compilation, and npm package publishing.
- **Positive**: Async support via `napi-rs`'s `AsyncTask` and thread-safe functions.
- **Negative**: Only targets Node.js. Python/Swift/Kotlin require a separate FFI bridge (uniffi or similar).
- **Negative**: `.node` binaries are platform-specific. Need CI matrix for linux-x64, linux-arm64, macos-x64, macos-arm64, win32-x64.

## References
- [napi-and-pubsub.md](../napi-and-pubsub.md)
- [OQ-11](../open-questions.md) — resolved by this ADR