---
status: reviewed
last_updated: 2026-06-02
---

# NAPI Wrapper & PubSub Event Target

## What

Two integration layers that enable TypeScript/JavaScript consumers to use wraith as a transport:

1. **NAPI wrapper** (`@alkdev/wraith`) — A Node.js native addon (via napi-rs) exposing `connect()` and `serve()` that return duplex streams
2. **PubSub event target** (`@alkdev/pubsub` adapter) — An implementation of the `TypedEventTarget` interface that routes events over wraith's SSH channel

## Why

The wraith Rust binary serves CLI users. But the broader ecosystem (pubsub, operations, agent spokes) is TypeScript-first. These integration layers let TypeScript code use wraith's transport without reimplementing SSH.

The NAPI surface is intentionally minimal — it exposes transport connections as duplex streams, not the full SSH protocol. The pubsub adapter wraps those streams with `EventEnvelope` serialization.

## Architecture

### NAPI Wrapper (napi-rs)

The wrapper uses napi-rs (ADR-015) and exposes two functions (ADR-016):

```typescript
// @alkdev/wraith (TypeScript side)

interface WraithConnectOptions {
  // TCP/TLS mode
  server?: string;           // e.g., "example.com:443"
  // iroh mode
  peer?: string;             // iroh endpoint ID (base58-encoded)
  // Transport
  transport: 'tcp' | 'tls' | 'iroh';
  // Auth
  identity?: string;         // path to SSH key, or Buffer with key data
  // TLS
  tlsServerName?: string;    // SNI hostname
  insecure?: boolean;        // accept self-signed certs
  // iroh
  irohRelay?: string;       // relay URL (default: n0)
  // Proxy
  proxy?: string;            // upstream SOCKS5/HTTP proxy URL
}

interface WraithServeOptions {
  // Transport
  transport: 'tcp' | 'tls' | 'iroh';
  // Auth
  hostKey?: string;          // path to SSH host key, or Buffer with key data
  authorizedKeys?: string;  // path to authorized_keys, or Buffer with key data
  certAuthority?: string;   // path to CA public key for cert-authority auth
  // TLS
  tlsCert?: string;          // path to TLS cert
  tlsKey?: string;           // path to TLS key
  acmeDomain?: string;      // ACME domain for auto-cert (ADR-008)
  // Listen
  listen?: string;           // listen address (default: 0.0.0.0:22)
  // iroh
  irohRelay?: string;       // relay URL (default: n0)
}

// Returns a Duplex stream for the SSH channel
function connect(options: WraithConnectOptions): Promise<Duplex>;

// Returns a server object with close() and connection events
function serve(options: WraithServeOptions): Promise<WraithServer>;

interface WraithServer {
  close(): Promise<void>;
  onConnection(callback: (stream: Duplex, info: ConnectionInfo) => void): void;
}
```

The NAPI layer is **transport-agnostic** — it doesn't know about pubsub's `EventEnvelope`. The pubsub adapter wraps the `Duplex` stream to implement `TypedEventTarget`. This separation ensures the NAPI wrapper is reusable for any stream-based protocol, not tied specifically to pubsub.

### NAPI `connect()` vs CLI `wraith connect`

The NAPI `connect()` function and the CLI `wraith connect` command are fundamentally different operations despite sharing the same name:

- **CLI `wraith connect`**: Starts a full SSH client session with a local SOCKS5 server and optional port forwards. It manages multiple SSH channels over a single session — the user routes traffic through it via SOCKS5 or forwarded ports.
- **NAPI `connect()`**: Opens a single SSH channel and returns it as a `Duplex` stream. No SOCKS5 server, no port forwarding. The caller reads and writes bytes directly. This is designed for the pubsub/programmatic use case where a single bidirectional byte stream is needed.

For SOCKS5 proxy functionality, use the CLI binary (`wraith connect`). The NAPI wrapper is for programmatic consumers that need a raw stream.

### Programmatic Configuration (ADR-011)

Both `connect()` and `serve()` accept options as plain objects. No file paths are mandatory — keys can be provided as `Buffer` data directly, making programmatic usage straightforward. Environment variables (`WRAITH_SERVER`, `WRAITH_IDENTITY`) provide convenience defaults.

Key material provided as `Buffer` must be in **OpenSSH key format** (the format used by `ssh-keygen`). Private keys: OpenSSH format (`-----BEGIN OPENSSH PRIVATE KEY-----`). Public keys: OpenSSH format (`ssh-ed25519 AAAA...`). PEM-encoded keys (PKCS#1, PKCS#8) are not supported.

### PubSub Event Target Adapter

This implements `TypedEventTarget` from `@alkdev/pubsub`:

```typescript
// @alkdev/pubsub (new adapter: event-target-wraith.ts)

export interface WraithEventTargetOptions {
  stream: Duplex;  // from @alkdev/wraith.connect() or serve()
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

The wraith server uses a reserved `direct_tcpip` destination (`wraith-control:0`) for the pubsub control channel (ADR-018). When a client connects to this destination:

1. The server's `channel_open_direct_ip` handler detects the reserved `wraith-control` target
2. Instead of opening a TCP connection, it bridges the channel to its local pubsub event bus
3. `EventEnvelope` JSON flows bidirectionally over the SSH channel

Users who prefer not to use the control channel can alternatively run a pubsub hub on a specific port and use standard port forwarding: `wraith connect --forward 9736:hub:9736`. This is a deployment choice, not a separate implementation — wraith's port forwarding works normally for any TCP service.

### Direction Agnostic

Because wraith supports both local and remote port forwarding, the event target works in either direction:

- **Worker connects to hub**: `wraith connect --forward 9736:hub:9736` then create WebSocket event target pointing at `ws://localhost:9736`
- **Hub connects to worker**: `wraith connect --remote-forward 9736:worker:9736` — same result, opposite initiator

The pubsub adapter doesn't care which side initiated the SSH session. It just needs a byte stream.

## Constraints

- The NAPI wrapper exposes duplex streams, not the full SSH channel API. Multiplexing is done at the pubsub layer.
- The pubsub wire protocol is length-prefixed JSON, matching the existing adapter pattern. Binary payloads should be base64-encoded in the `EventEnvelope.payload`.
- The NAPI binary size will be ~5-10MB (includes russh + tokio + cryptography). The `iroh` feature adds significant size; it should be an optional feature.
- Keys can be provided as file paths or `Buffer` data, supporting both CLI and programmatic usage patterns (ADR-011).

## Open Questions

None — all resolved.

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [007](decisions/007-napi-single-stream.md) | NAPI exposes single duplex stream | No SSH multiplexing in JS, pubsub handles it |
| [011](decisions/011-no-ssh-config-programmatic-api.md) | Programmatic-first API | No file-based config; options are structs or env vars |
| [015](decisions/015-napi-rs-for-ffi-bridge.md) | napi-rs for FFI | Standard Node.js native addon tooling |
| [016](decisions/016-napi-expose-connect-and-serve.md) | Both connect() and serve() | NAPI exposes client and server sides from the start |
| [018](decisions/018-control-channel-for-pubsub.md) | Control channel for pubsub | Reserved `wraith-control` destination for event bus |