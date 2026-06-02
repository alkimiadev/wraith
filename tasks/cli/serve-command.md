---
id: cli/serve-command
name: Implement `wraith serve` CLI subcommand with clap
status: pending
depends_on:
  - server/serve-loop
scope: moderate
risk: low
impact: component
level: implementation
---

## Description

Implement the `wraith serve` CLI subcommand using `clap` with derive macros. This translates `ServeOptions` into CLI flags and runs the server. All options from server.md CLI interface must be supported.

Environment variable defaults: none mandated for serve, but consistent with programmatic-first API.

The binary is the `wraith` crate at `crates/wraith/src/main.rs`.

## Acceptance Criteria

- [ ] `crates/wraith/src/main.rs` defines CLI with clap derive: `wraith` with `serve` and `connect` subcommands (connect stub for now)
- [ ] `wraith serve` subcommand flags match server.md CLI interface exactly: `--key`, `--authorized-keys`, `--cert-authority`, `--transport`, `--listen`, `--tls-cert`, `--tls-key`, `--acme-domain`, `--stealth`, `--proxy`, `--iroh-relay`, `--max-connections-per-ip`, `--max-auth-attempts`
- [ ] `--key` is required (no default)
- [ ] `--transport` defaults to `tcp`
- [ ] `--listen` defaults to `0.0.0.0:22`
- [ ] `--stealth` validates that `--transport tls` is set; error otherwise
- [ ] `--transport iroh` prints endpoint ID on startup
- [ ] `--acme-domain` requires `acme` feature (compile-time or runtime error if missing)
- [ ] Key inputs accept file paths (strings); in-memory key data is a library/API concern, not CLI
- [ ] CLI translates args into `ServeOptions` and calls `Server::new(opts).run().await`
- [ ] Errors reported to stderr with non-zero exit code
- [ ] `cargo run -p wraith -- serve --help` shows all flags with descriptions

## References

- docs/architecture/server.md — CLI Interface section with all flags
- docs/architecture/overview.md — "A single binary with subcommands"

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion