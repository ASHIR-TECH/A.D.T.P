# Agent Card Schema Specification

ADTP v0.1.0 — Agent Card schema for peer identity and capability advertisement.

## Overview

An Agent Card is a signed JSON document served by an ADTP peer at
`/.well-known/adtp-agent.json`. It allows other peers to discover the
peer's identity, capabilities, and relay configuration.

## Endpoint

```
GET http://<host>:<agent-card-port>/.well-known/adtp-agent.json
```

Response: `200 OK` with `Content-Type: application/json`

## Schema

```json
{
  "adtp_version": 1,
  "peer_id": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
  "public_key_hex": "e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
  "display_name": "Alice's Workstation",
  "host": "10.0.0.1",
  "port": 40001,
  "max_file_size": 10737418240,
  "supported_mime_types": [
    "text/plain",
    "application/pdf",
    "image/png"
  ],
  "supports_tasks": true,
  "supports_multihop": true,
  "relay_of": [
    "deadbeef...",
    "cafebabe..."
  ],
  "issued_at": "2026-06-23T12:00:00Z",
  "signature": "f1a2b3c4... (64-byte hex-encoded ed25519 signature)"
}
```

## Type Definitions

### adtp_version
- **Type:** `u8` (integer)
- **Range:** `1`
- Current protocol version. Reserved for future protocol evolution.

### peer_id
- **Type:** `string`
- **Format:** 32-character lowercase hex string
- **Size:** 16 bytes decoded
- The BLAKE3 hash of the ed25519 verifying key, truncated to 16 bytes.

### public_key_hex
- **Type:** `string`
- **Format:** 64-character lowercase hex string
- **Size:** 32 bytes decoded
- The raw ed25519 verifying key bytes.

### display_name
- **Type:** `string` or `null`
- An optional human-readable label for the peer.

### host
- **Type:** `string`
- An IP address or DNS name where the peer's ADTP QUIC listener is reachable.

### port
- **Type:** `u16` (integer)
- **Range:** `1–65535`
- **Default:** `40001`
- The UDP port of the peer's QUIC listener.

### max_file_size
- **Type:** `u64` (integer)
- Maximum accepted file size in bytes for a single transfer.

### supported_mime_types
- **Type:** `string[]`
- List of MIME types the peer can handle. An empty list means the peer
  accepts all types.

### supports_tasks
- **Type:** `bool`
- If `true`, the peer implements the Task API (Phase 8).

### supports_multihop
- **Type:** `bool`
- If `true`, the peer participates in multi-hop relay (Phase 9).

### relay_of
- **Type:** `string[]`
- List of hex-encoded peer IDs this peer is willing to relay packets for.
  An empty list means the peer relays for no one (but may still forward
  its own packets through others).

### issued_at
- **Type:** `string`
- **Format:** RFC 3339 timestamp
- Example: `2026-06-23T12:00:00Z`
- The moment the card was signed, used for staleness checks.

### signature
- **Type:** `string`
- **Format:** 128-character lowercase hex string
- **Size:** 64 bytes decoded
- ed25519 signature over the canonical JSON of all other fields.

## Signature Verification

### Signing (issuer side)

1. Build the unsigned payload (`UnsignedCard`) containing all fields
   EXCEPT `signature`.
2. Serialize `UnsignedCard` to canonical JSON using `serde_json::to_string`
   (field order defined by `#[derive(Serialize)]`).
3. Sign the canonical JSON bytes with the peer's ed25519 `SigningKey`.
4. Encode the 64-byte signature as lowercase hex.

### Verification (consumer side)

1. Fetch the Agent Card JSON.
2. Parse into an `AgentCard` struct.
3. Reconstruct the `UnsignedCard` from all fields except `signature`.
4. Serialize `UnsignedCard` to canonical JSON.
5. Decode `public_key_hex` to 32 bytes and create a `VerifyingKey`.
6. Decode `signature` to 64 bytes.
7. Call `verifying_key.verify(canonical_bytes, &signature)`.
8. If verification fails, discard the card.

## Caching

- Consumers SHOULD cache Agent Cards for the duration of a session.
- Consumers MAY re-fetch on connection failure or staleness heuristics.
- Cards contain no explicit expiry; rely on `issued_at` for staleness.

## Security Considerations

- Cards are self-signed. Trust is bootstrapped through the `TrustStore`
  (peer public keys registered out of band).
- The Agent Card HTTP server binds to `127.0.0.1` in the default
  configuration. For production, serve behind a TLS-terminating reverse
  proxy.
- A compromised signing key invalidates all previously issued cards for
  that peer ID. Rotate by generating a new identity and re-registering.
