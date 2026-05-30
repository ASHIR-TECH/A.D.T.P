# Security Policy

ADTP is a file-transfer protocol, so security review is mandatory for networking, cryptography, file-system writes, audit logs, and configuration changes.

## Supported Versions

| Version | Status |
| --- | --- |
| `0.1.x` | Active development, security fixes accepted |

## Reporting a Vulnerability

Please do not open a public issue for exploitable vulnerabilities. Report privately to the maintainers with:

- Affected crate/module.
- Reproduction steps.
- Impact description.
- Suggested mitigation if known.
- Whether the issue requires coordinated disclosure.

## Current Security Model

```mermaid
flowchart LR
    File[Raw File] --> Hash[BLAKE3 Hash]
    File --> Encrypt[AES-256-GCM Encrypt]
    Hash --> Header[Packet Header]
    Encrypt --> Payload[Encrypted Payload]
    Header --> Packet[ADTP Packet]
    Payload --> Packet
    Packet --> QUIC[QUIC/TLS Transport]
    QUIC --> Receiver[Receiver]
    Receiver --> Decrypt[Decrypt]
    Decrypt --> Verify[Verify BLAKE3]
    Verify --> Disk[Write to Disk]
    Verify --> Audit[Audit Chain]
```

## Important Phase 1 Limitations

- Daemon flow currently uses a zero session key as a development placeholder.
- Self-signed TLS certificates are generated at runtime for development.
- Signature fields exist but production signing/verification is not yet enforced end-to-end.
- Context extraction is not implemented until a later phase.

These are known development limitations, not production guarantees.

## Security Requirements for Changes

- Do not write received files before integrity verification passes.
- Do not remove audit logging for transfer/session events.
- Do not introduce plaintext payload transfer paths.
- Do not log raw file contents, private keys, or derived session keys.
- Validate file sizes before processing large payloads.
- Treat config paths as untrusted input.

## Dependency Auditing

Run:

```bash
cargo audit
```

Current known advisory warnings may include unmaintained crates inherited from Phase 0 choices. Warnings must be reviewed before release.

## Threat Model Summary

| Threat | Current Mitigation |
| --- | --- |
| Payload tampering | AES-GCM authentication and BLAKE3 content hash verification |
| Packet corruption | Header validation and deserialization checks |
| Audit tampering | Linked BLAKE3 audit chain |
| Oversized files | File-size guards in protocol/intake layers |
| Network interception | QUIC/TLS transport; production cert validation still planned |
| Replay/identity attacks | Full session handshake/signatures planned for later phase |
