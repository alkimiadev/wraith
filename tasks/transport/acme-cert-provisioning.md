---
id: transport/acme-cert-provisioning
name: Implement ACME Lets Encrypt certificate provisioning (feature-gated acme)
status: pending
depends_on:
  - transport/tls-transport
scope: moderate
risk: high
impact: component
level: implementation
---

## Description

Implement automatic TLS certificate provisioning via ACME (Let's Encrypt). Two modes per ADR-008:

1. **Domain-based ACME** (`--acme-domain`): Standard flow with HTTP-01 or TLS-ALPN-01 challenges. Domain-bound, auto-renewing.
2. **IP-based ACME**: Short-lived certs via TLS-ALPN-01 on port 443. No domain needed.

Uses `rustls-acme` (pure Rust) to avoid external certbot dependency. Feature-gated behind `acme` (implies `tls`).

This integrates with `TlsAcceptor` by providing ACME-resolved certificates instead of manual cert/key files.

## Acceptance Criteria

- [ ] `crates/wraith-core/src/transport/acme.rs` (behind `#[cfg(feature = "acme")]`)
- [ ] Feature `acme` implies `tls` in Cargo.toml
- [ ] `AcmeCertProvider` struct accepts: domain (domain-based) or IP mode flag
- [ ] Domain-based mode: uses `rustls-acme` with HTTP-01/TLS-ALPN-01 challenge responder
- [ ] IP-based mode: uses `rustls-acme` with TLS-ALPN-01 on port 443
- [ ] `AcmeCertProvider` produces a `rustls::ServerConfig` that `TlsAcceptor` can use
- [ ] Certificate auto-renewal handled by `rustls-acme` background task
- [ ] `TlsAcceptor` updated to accept either manual certs OR an `AcmeCertProvider`
- [ ] Integration with `TlsAcceptor::bind_acme()` or similar constructor
- [ ] Unit tests for ACME config construction (challenge responder setup)
- [ ] Integration test: ACME cert provisioning with Let's Encrypt staging (marked `#[ignore]` for CI)

## References

- docs/architecture/server.md — TLS certificate provisioning modes
- docs/architecture/decisions/008-acme-lets-encrypt.md — ACME design, rustls-acme choice
- docs/architecture/transport.md — feature flags, TLS transport constraints

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion