---
id: meta/napi-layer
name: Complete NAPI layer — project setup, connect(), serve()
status: pending
depends_on:
  - napi/project-setup
  - napi/connect-function
  - napi/serve-function
scope: moderate
risk: high
impact: phase
level: planning
---

## Description

Meta task that clusters NAPI tasks. Once complete, the `@alkdev/wraith` Node.js native addon provides `connect()` and `serve()` returning duplex streams for TypeScript consumers.

## Acceptance Criteria

- [ ] All NAPI tasks completed
- [ ] `connect()` returns Duplex stream, no SOCKS5, no port forwarding
- [ ] `serve()` returns WraithServer with close() and onConnection events
- [ ] Key material from Buffer (in-memory) and file paths both work
- [ ] JS-to-Rust and Rust-to-JS error marshalling works correctly

## References

- docs/architecture/napi-and-pubsub.md

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion