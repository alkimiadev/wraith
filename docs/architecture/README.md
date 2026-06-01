---
status: draft
last_updated: 2026-06-01
---

# Wraith Architecture

## Current State

Pre-implementation. Feasibility assessment complete (see research/ssh-tunnel-vpn-alternative-feasibility.md). Architecture specification in progress.

## Architecture Documents

| Document | Status | Description |
|----------|--------|-------------|
| [overview.md](overview.md) | draft | Package purpose, exports, dependencies |
| [transport.md](transport.md) | draft | Transport abstraction: TCP, TLS, iroh |
| [client.md](client.md) | draft | Client connection, SOCKS5, port forwarding |
| [server.md](server.md) | draft | Server acceptance, channel handling, proxy |
| [tun-shim.md](tun-shim.md) | draft | Privileged TUN interface wrapper (separate process) |
| [napi-and-pubsub.md](napi-and-pubsub.md) | draft | NAPI wrapper and pubsub event target adapter |

## ADR Table

| ADR | Title | Status |
|-----|-------|--------|
| [001](decisions/001-pluggable-transport.md) | Pluggable transport via `AsyncRead+AsyncWrite` trait | Accepted |
| [002](decisions/002-tun-separate-process.md) | TUN shim as separate process | Accepted |
| [003](decisions/002-iroh-stream-join.md) | iroh stream via `tokio::io::join` | Accepted |
| [004](decisions/004-ssh-over-transport.md) | SSH runs over transport, not alongside | Accepted |
| [005](decisions/005-socks5-before-tun.md) | SOCKS5 as primary interface, TUN as add-on | Accepted |

## Open Questions

See [open-questions.md](open-questions.md)

## Lifecycle Definitions

| Status | Meaning | Transitions |
|--------|---------|-------------|
| `draft` | Under active development. May change significantly. | → `reviewed` when open questions resolved |
| `reviewed` | Architecture final. Implementation may begin. Changes require review. | → `stable` when implementation verified |
| `stable` | Locked. Changes require review and may warrant an ADR. | → `deprecated` when superseded |
| `deprecated` | Superseded. Kept for reference. | Removed when no longer referenced |