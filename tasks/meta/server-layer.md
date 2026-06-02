---
id: meta/server-layer
name: Complete server layer — handler, channel proxy, stealth, rate limiting, control channel, serve loop
status: pending
depends_on:
  - server/handler
  - server/channel-proxy
  - server/stealth-mode
  - server/rate-limiting-and-logging
  - server/control-channel
  - server/serve-loop
scope: system
risk: high
impact: phase
level: planning
---

## Description

Meta task that clusters all server module tasks. Once complete, the server accepts SSH connections via any transport, authenticates clients, proxies channel traffic to TCP targets (directly or via proxy), handles stealth mode, rate limits connections, routes reserved `wraith-` destinations, and shuts down gracefully.

## Acceptance Criteria

- [ ] All server tasks completed
- [ ] Server handles SSH connections over TCP, TLS, and iroh transports
- [ ] Authentication via Ed25519 keys and cert-authority
- [ ] Channel proxying with direct, SOCKS5, and HTTP CONNECT outbound modes
- [ ] Stealth mode detects SSH vs HTTP and returns fake nginx 404
- [ ] Rate limiting and structured logging
- [ ] Control channel routing for `wraith-*` destinations
- [ ] Graceful shutdown

## References

- docs/architecture/server.md

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion