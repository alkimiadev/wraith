---
status: draft
last_updated: 2026-06-01
---

# TUN Shim

## What

A separate process (`wraith-tun`) that creates a TUN interface, reads IP packets from it, and forwards them through the core wraith client's SOCKS5 port. Requires root or `CAP_NET_ADMIN`.

## Why

The core wraith binary must never require root. TUN interfaces need elevated privileges. By separating TUN into its own minimal process, we:

- Minimize the root-required code surface (auditable in an afternoon)
- Keep the core binary unprivileged for SOCKS5 and port forwarding
- Allow the TUN shim to crash without affecting the SSH session
- Match the proven tun2proxy architecture

(ADR-002)

## Architecture

```
┌─────────────────┐     ┌──────────────────────────────┐
│   wraith-tun     │     │      wraith connect           │
│   (root)         │     │      (unprivileged)            │
│                  │     │                                │
│  ┌────────────┐ │     │  ┌──────────────────────────┐ │
│  │ TUN Device │ │     │  │ SOCKS5 Server             │ │
│  │ (tun-rs)   │◄├─────┤├►│ :1080                    │ │
│  │ 10.0.0.1/24│ │     │  └──────────────────────────┘ │
│  └────────────┘ │     │                                │
│                  │     │  ┌──────────────────────────┐ │
│  Route all      │     │  │ SSH Client (russh)        │ │
│  traffic via    │     │  │ connect via Transport     │ │
│  TUN device    │     │  └──────────────────────────┘ │
└─────────────────┘     └──────────────────────────────┘
```

### Data Flow

1. OS routing table sends all traffic through TUN device `tun0`
2. `wraith-tun` reads IP packets from TUN device
3. `wraith-tun` extracts destination IP:port from each packet
4. `wraith-tun` connects to `127.0.0.1:1080` (wraith SOCKS5) with `SOCKS5h` (domain resolution by proxy)
5. `wraith-tun` proxies the TCP connection through SOCKS5
6. wraith's SOCKS5 server opens an SSH direct-tcpip channel to the destination
7. Bytes flow: application → TUN → SOCKS5 → SSH channel → server → target

### Virtual DNS

The TUN shim implements virtual DNS (same approach as tun2proxy):

- DNS queries to port 53 arriving at the TUN device are intercepted
- Query names are mapped to fake IPs from `198.18.0.0/15`
- Connections to fake IPs are resolved to the original domain name via SOCKS5h
- This prevents DNS leaks (all DNS resolution happens server-side)

### CLI Interface

```bash
# Basic TUN mode (uses wraith's SOCKS5 on 127.0.0.1:1080)
sudo wraith-tun --socks5 127.0.0.1:1080

# With custom TUN address
sudo wraith-tun --socks5 127.0.0.1:1080 --tun-addr 10.0.0.1/24

# With DNS configuration
sudo wraith-tun --socks5 127.0.0.1:1080 --dns virtual

# Unprivileged mode (creates network namespace)
wraith-tun --socks5 127.0.0.1:1080 --unshare
```

### Unprivileged Mode

The `--unshare` flag creates a new network namespace, sets up the TUN device inside it, and maintains connectivity to the SOCKS5 proxy via the global namespace. This allows running without root, using only `CAP_NET_ADMIN` capability or namespace creation permissions.

## Scope

This is Phase 3 of the implementation plan. The core (`wraith serve`, `wraith connect` with SOCKS5 and port forwarding) comes first. TUN is an add-on.

### What `wraith-tun` Does NOT Do

- It does not manage SSH sessions
- It does not know about transports
- It does not handle authentication
- It does not read SSH keys

It only: reads packets from TUN, forwards to SOCKS5. That's it.

## Constraints

- Requires root or `CAP_NET_ADMIN` (or `--unshare` namespace isolation)
- IPv4 only in initial release, IPv6 follow-up
- UDP over TCP (DNS queries are handled via SOCKS5h, other UDP is dropped in initial release)
- Approximately 200-500 lines of Rust for the initial implementation

## Open Questions

- **OQ-03**: Windows TUN support scope (wintun.dll dependency)
- **OQ-09**: Whether to use tun2proxy's `ip-stack` crate for TCP reconstruction or implement a simpler packet-level approach

## Design Decisions

| ADR | Decision | Summary |
|-----|----------|---------|
| [002](decisions/002-tun-separate-process.md) | TUN separate process | Core never needs root, TUN is thin wrapper |
| [005](decisions/005-socks5-before-tun.md) | SOCKS5 first | TUN forwards to SOCKS5, not to SSH directly |