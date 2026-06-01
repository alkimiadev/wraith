---
status: draft
last_updated: 2026-06-01
---

# Client

## What

The wraith client establishes an SSH session to a server (via pluggable transport) and exposes local interfaces for routing traffic through that session: SOCKS5 proxy, port forwarding, and eventually TUN.

## Why

Users need a way to route traffic through the SSH tunnel. SOCKS5 is the primary interface — it's standard, well-supported by browsers and CLI tools, and needs no privileges. Port forwarding (`-L` / `-R` style) covers specific service access like Postgres or Redis. TUN covers full-system VPN-like behavior.

## Architecture

### Client Components

```
┌────────────────────────────────────────────────────────┐
│                     wraith connect                      │
│                                                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │ SOCKS5   │ │ Port     │ │ Remote   │ │ (TUN     │ │
│  │ Server   │ │ Forward  │ │ Forward  │ │  shim)   │ │
│  │ :1080    │ │ -L spec  │ │ -R spec  │ │ separate │ │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └──────────┘ │
│       │             │             │                     │
│       ▼             ▼             ▼                     │
│  ┌─────────────────────────────────┐                   │
│  │        Channel Manager          │                   │
│  │  (opens direct-tcpip,           │                   │
│  │   forwarded-tcpip streams)      │                   │
│  └──────────────┬──────────────────┘                   │
│                  │                                      │
│  ┌──────────────▼──────────────────┐                   │
│  │       SSH Client (russh)        │                   │
│  │   Handle<ClientHandler>         │                   │
│  └──────────────┬──────────────────┘                   │
│                  │                                      │
│  ┌──────────────▼──────────────────┐                   │
│  │         Transport                │                   │
│  │   (Tcp / Tls / Iroh)            │                   │
│  └──────────────────────────────────┘                   │
└────────────────────────────────────────────────────────┘
```

### SOCKS5 Server

The primary client interface. Listens on a local port (default `127.0.0.1:1080`), accepts SOCKS5 connections, and for each connection:

1. Reads the SOCKS5 handshake (auth method negotiation, target address)
2. Opens a `channel_open_direct_tcpip(target_host, target_port, originator_addr, originator_port)` on the SSH session
3. Converts the SSH channel to a stream via `channel.into_stream()`
4. Runs `tokio::io::copy_bidirectional(&mut local_socket, &mut ssh_stream)` to proxy data

Supports SOCKS5h (domain names resolved server-side) by default. This prevents DNS leaks.

### Port Forwarding

Local port forwards (`-L local_addr:local_port:remote_host:remote_port`):

1. Bind `TcpListener` on `local_addr:local_port`
2. For each accepted connection, open `channel_open_direct_tcpip(remote_host, remote_port, ...)`
3. Proxy bytes bidirectionally via `copy_bidirectional`

Remote port forwards (`-R remote_addr:remote_port:local_host:local_port`):

1. Send `tcpip_forward(remote_addr, remote_port)` to request the server listen on a port
2. When the handler receives `server_channel_open_forwarded_tcpip`, connect to `local_host:local_port`
3. Proxy bytes bidirectionally

### Channel Manager

The channel manager owns the `Arc<client::Handle<ClientHandler>>` and provides methods:

- `open_direct_tcpip(host, port)` — open a tunnel channel to a remote host
- `open_streamlocal(socket_path)` — open a tunnel to a Unix socket
- `request_tcpip_forward(addr, port)` — request remote listening
- `cancel_tcpip_forward(addr, port)` — cancel remote listening

It also handles reconnection: if `handle.is_closed()` returns true, attempt reconnection with exponential backoff.

### Reconnection

On transport failure:

1. Detect via `handle.is_closed()` or transport read error
2. Exponential backoff reconnect (1s, 2s, 4s, ... max 30s)
3. Re-establish transport connection
4. Re-authenticate SSH session
5. Notify SOCKS5 server and port forwards (in-flight connections fail, new connections work)

Existing TCP connections through the tunnel are lost on reconnect. This is acceptable — same as any VPN.

### CLI Interface

```bash
# Basic connection (TCP, default port 22)
wraith connect --server example.com --identity ~/.ssh/id_ed25519

# With TLS
wraith connect --server example.com:443 --transport tls --identity ~/.ssh/id_ed25519

# With iroh (no public IP needed)
wraith connect --peer <endpoint-id> --transport iroh --identity ~/.ssh/id_ed25519

# SOCKS5 on custom port
wraith connect --server example.com --socks5 127.0.0.1:1080 --identity ~/.ssh/id_ed25519

# With port forwards
wraith connect --server example.com --forward 5432:db.internal:5432 --forward 6379:redis.internal:6379

# All options
wraith connect \
  --server <addr> \          # TCP server address (required for tcp/tls)
  --peer <endpoint-id> \    # iroh peer ID (required for iroh)
  --transport tcp|tls|iroh \ # Transport mode
  --identity <path> \       # SSH private key path
  --socks5 <addr:port> \    # SOCKS5 listen address (default: 127.0.0.1:1080)
  --forward <spec> \        # Port forward spec (repeatable)
  --remote-forward <spec> \ # Remote port forward spec (repeatable)
  --proxy <url>              # Upstream proxy (SOCKS5/HTTP CONNECT)
```

## Constraints

- SOCKS5 is always enabled when `wraith connect` runs (it's the primary interface). Port forwards are optional.
- The client does not know or log what destinations are accessed. The SOCKS5 server connects and proxies — no logging of SOCKS5 request targets.
- Authentication is Ed25519 public key only by default. Password auth supported but not recommended. (OQ-04)
- Only one SSH session per `wraith connect` process. Multiple sessions = multiple processes (or a future multiplexer).

## Open Questions

- **OQ-04**: Authentication beyond Ed25519 keys
- **OQ-06**: Whether to support SSH config file parsing (`~/.ssh/config`) for default host/key/port settings

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [005](decisions/005-socks5-before-tun.md) | SOCKS5 first | SOCKS5 is the primary interface, TUN forwards to it |