---
status: draft
last_updated: 2026-06-01
---

# NAPI Wrapper & PubSub Event Target

## What

Two integration layers that enable TypeScript/JavaScript consumers to use wraith as a transport:

1. **NAPI wrapper** (`@alkdev/wraith`) — A minimal Node.js native addon exposing `connect()` and `serve()` that return duplex streams
2. **PubSub event target** (`@alkdev/pubsub` adapter) — An implementation of the `TypedEventTarget` interface that routes events over wraith's SSH channel

## Why

The wraith Rust binary serves CLI users. But the broader ecosystem (pubsub, operations, agent spokes) is TypeScript-first. These integration layers let TypeScript code use wraith's transport without reimplementing SSH.

The NAPI surface is intentionally tiny — it exposes the transport connection, not the full SSH protocol. The pubsub adapter is also minimal — it implements `TypedEventTarget` and serializes `EventEnvelope` JSON over the stream.

## Architecture

### NAPI Wrapper

The wrapper exposes a single function that establishes a wraith connection and returns a Node.js `Duplex` stream:

```typescript
// @alkdev/wraith (TypeScript side)

interface WraithConnectOptions {
  // TCP/TLS mode
  server?: string;           // e.g., "example.com:443"
  // iroh mode
  peer?: string;             // iroh EndpointId (hex)
  // Transport
  transport: 'tcp' | 'tls' | 'iroh';
  // Auth
  identity?: string;         // path to SSH key
  // TLS
  tlsServerName?: string;    // SNI hostname
  insecure?: boolean;         // accept self-signed certs
  // iroh
  irohRelay?: string;        // relay URL (default: n0)
}

function connect(options: WraithConnectOptions): Duplex;
```

The `Duplex` stream carries raw SSH channel data. On the Rust side, the NAPI function:

1. Creates a transport (TCP/TLS/iroh) based on options
2. Establishes an SSH session via `client::connect_stream()`
3. Opens a single `direct_tcpip` channel to a well-known destination (or uses a control protocol)
4. Returns the channel as a NAPI `Buffer` stream

**Key design decision**: The NAPI wrapper does NOT expose the full SSH channel multiplexing API. It returns one duplex stream. If the TypeScript consumer needs multiple logical channels, it multiplexes them itself (e.g., via pubsub's event routing).

### PubSub Event Target Adapter

This implements `TypedEventTarget` from `@alkdev/pubsub`:

```typescript
// @alkdev/pubsub (new adapter: event-target-wraith.ts)

export interface WraithEventTargetOptions {
  stream: Duplex;  // from @alkdev/wraith.connect()
}

export interface WraithEventTarget<TEvent extends TypedEvent>
  extends TypedEventTarget<TEvent> {
  close(): void;
}

export function createWraithEventTarget<TEvent extends TypedEvent>(
  options: WraithEventTargetOptions
): WraithEventTarget<TEvent>;
```

Wire protocol (same as other pubsub adapters):

- **Framing**: 4-byte big-endian length prefix + JSON payload
- **Payload**: `EventEnvelope` JSON (`{ type, id, payload }`)
- **Control**: `__subscribe` / `__unsubscribe` messages for topic-based routing
- **Direction**: Bidirectional — `dispatchEvent` sends, `addEventListener` subscribes and receives

### On the Server Side

The wraith server exposes a "control channel" — a special `direct_tcpip` destination (e.g., `wraith-control:0`) that routes to the pubsub event bus instead of a TCP target. When a client connects to this destination:

1. Server's `channel_open_direct_tcpip` handler detects the special target
2. Instead of opening a TCP connection, it bridges the channel to its local pubsub event bus
3. `EventEnvelope` JSON flows bidirectionally over the SSH channel

Alternatively, the server can listen on a specific port (e.g., `9736`) for the hub's WebSocket server, and wraith simply port-forwards that port.

### Direction Agnostic

Because wraith supports both local and remote port forwarding, the event target works in either direction:

- **Worker connects to hub**: `wraith connect --forward 9736:hub:9736` then create WebSocket event target pointing at `ws://localhost:9736`
- **Hub connects to worker**: `wraith connect --remote-forward 9736:worker:9736` — same result, opposite initiator

The pubsub adapter doesn't care which side initiated the SSH session. It just needs a byte stream.

## Constraints

- The NAPI wrapper exposes a single duplex stream, not the full SSH channel API. Multiplexing is done at the pubsub layer.
- The pubsub wire protocol is length-prefixed JSON, matching the existing adapter pattern. Binary payloads should be base64-encoded in the `EventEnvelope.payload`.
- The NAPI binary size will be ~5-10MB (includes russh + tokio + cryptography). The `iroh` feature adds significant size; it should be an optional feature.

## Open Questions

- **OQ-10**: Whether the NAPI wrapper should expose raw channel access or a higher-level "send JSON, receive JSON" API
- **OQ-11**: Whether to use napi-rs or uniffi for the FFI bridge (napi-rs is more established for Node.js, uniffi supports more targets)

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [007](decisions/007-napi-single-stream.md) | NAPI exposes single duplex stream | No SSH multiplexing in JS, pubsub handles it |