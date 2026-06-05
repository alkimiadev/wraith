# Wraith

> **This project has been renamed to [Alknet](https://git.alk.dev/alkdev/alknet).**
>
> All future development continues under the **Alknet** name at:
>
> - **Primary**: <https://git.alk.dev/alkdev/alknet>
> - **Mirror**: <https://github.com/alkimiadev/alknet>
>
> This repository is archived. No further changes will be made here. Please
> update your dependencies and references to the new repository. The code, crate
> names, CLI binary name, and all identifiers will be updated from `wraith` to
> `alknet` (e.g. `wraith-core` → `alknet-core`, `wraith serve` → `alknet serve`).
>
> The license (MIT OR Apache-2.0) remains the same.

---

A self-hostable SSH-based tunnel tool that provides VPN-like functionality without being a VPN protocol.

## What it does

- **Private tunneling** — Route traffic to internal services (Postgres, Redis, APIs) over SSH
- **Censorship circumvention** — SSH over TLS on port 443 is indistinguishable from HTTPS to DPI
- **NAT traversal** — The iroh transport enables peer-to-peer connections without public IPs or port forwarding
- **Service mesh connectivity** — Lightweight transport layer for event systems via reserved destinations

The core insight: SSH tunnels work because SSH is fundamental infrastructure. Blocking it breaks the internet.

## Quick start

See the [Alknet repository](https://git.alk.dev/alkdev/alknet) for current build and usage instructions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.