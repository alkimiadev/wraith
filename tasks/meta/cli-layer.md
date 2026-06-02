---
id: meta/cli-layer
name: Complete CLI layer — wraith serve and wraith connect commands
status: pending
depends_on:
  - cli/serve-command
  - cli/connect-command
scope: moderate
risk: low
impact: phase
level: planning
---

## Description

Meta task that clusters CLI tasks. Once complete, the `wraith` binary has both `serve` and `connect` subcommands with all flags matching the architecture specs.

## Acceptance Criteria

- [ ] Both CLI tasks completed
- [ ] `wraith serve --help` and `wraith connect --help` match architecture spec flag lists
- [ ] End-to-end: `wraith serve` + `wraith connect` establishes working SSH tunnel

## References

- docs/architecture/client.md, docs/architecture/server.md

## Notes

> To be filled by implementation agent

## Summary

> To be filled on completion