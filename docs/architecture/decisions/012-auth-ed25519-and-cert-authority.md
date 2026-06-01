# ADR-012: Ed25519 Keys + OpenSSH Certificate Authority, No Password Auth

## Status
Accepted

## Context
SSH authentication has several options:
- **Ed25519 public key**: The default, already specified. Each user has a keypair; the server has an `authorized_keys` file.
- **Password authentication**: Convenient for quick setups but less secure (susceptible to brute force, credential reuse).
- **OpenSSH certificate authority (cert-authority)**: A CA signs user certificates. The server trusts the CA instead of individual keys. Much easier for multi-user setups — add one CA line to `authorized_keys` instead of every user's public key. Also supports certificate expiry and restrictions.

The question is which auth methods to support and prioritize.

## Decision

**Primary: Ed25519 public key** (already specified, no change).

**Important: OpenSSH certificate authority**. Support `cert-authority` entries in `authorized_keys` files. When a user presents a certificate signed by a trusted CA, the server validates the certificate (signature, expiry, permissions) and accepts it. This is critical for multi-user deployments where managing individual keys is impractical.

**Not supported: Password authentication over SSH channels.** Password auth over an SSH tunnel (i.e., the SOCKS5 proxy requiring a password) is not in scope. Password auth over SSH itself is rejected because:
- It's less secure than key-based auth
- It's susceptible to brute force (fail2ban can mitigate, but keys eliminate the problem)
- It's not needed when cert-authority provides easy multi-user management
- If a local SOCKS5 proxy is desired with its own auth, that's a separate concern

The server's `authorized_keys` file format follows OpenSSH conventions:
- Regular keys: `ssh-ed25519 AAAA... user@host`
- CA trusts: `cert-authority ssh-ed25519 AAAA... CA name`
- Principals: `cert-authority,permit-port-forwarding ssh-ed25519 AAAA... CA name`

## Consequences
- **Positive**: Multi-user deployments are manageable — one CA entry instead of N key entries.
- **Positive**: Certificates can carry expiry dates and restrictions (permit-port-forwarding, no-pty, source-address).
- **Positive**: No password brute force risk. fail2ban still needed for connection-level abuse, but not for auth-level password guessing.
- **Positive**: `russh` supports OpenSSH certificate verification natively.
- **Negative**: Setting up a CA requires initial key management tooling (`ssh-keygen -s`).
- **Negative**: Users who want a quick "just let me in" experience need to generate keys first. Not a significant barrier for the target audience (self-hosting, ops).

## References
- [client.md](../client.md)
- [server.md](../server.md)
- [OQ-04](../open-questions.md) — resolved by this ADR