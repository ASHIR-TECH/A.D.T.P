# A2A Transport Binding Specification

ADTP v0.1.0 — Agent-to-Agent Transport Binding for multi-hop relay.

## Overview

The A2A Transport Binding extends ADTP with agent-to-agent communication
capabilities. Agents discover each other via signed Agent Cards served at
`/.well-known/adtp-agent.json` and route packets through intermediate relay
peers. Each hop performs loop detection, hop-limit enforcement, and per-hop
trust verification.

## Agent Card Discovery

Every ADTP peer that supports A2A MUST expose an Agent Card at
`http://<host>:<agent-card-port>/.well-known/adtp-agent.json`.

| Field | Type | Description |
|-------|------|-------------|
| `adtp_version` | `u8` | Protocol version |
| `peer_id` | `string` | Hex-encoded 16-byte peer ID |
| `public_key_hex` | `string` | Hex-encoded ed25519 public key |
| `display_name` | `string?` | Human-readable name |
| `host` | `string` | Publicly reachable host |
| `port` | `u16` | ADTP QUIC listening port |
| `max_file_size` | `u64` | Maximum accepted file size in bytes |
| `supported_mime_types` | `string[]` | List of supported MIME types |
| `supports_tasks` | `bool` | Whether the peer implements the Task API |
| `supports_multihop` | `bool` | Whether the peer acts as a relay |
| `relay_of` | `string[]` | Peer IDs this peer is willing to relay for |
| `issued_at` | `string` | RFC 3339 issuance timestamp |
| `signature` | `string` | Hex-encoded ed25519 signature over the canonical JSON |

### Signature Scheme

The issuer serializes ALL fields except `signature` as canonical JSON
(no whitespace, sorted keys), then signs with the ed25519 signing key
corresponding to `public_key_hex`. Verifiers reconstruct the unsigned
payload and verify against the embedded signature.

## ADTP URI Scheme

```
adtp://<peer_id>@<host>:<port>/<path>
```

- `peer_id` — Hex-encoded 16-byte peer identifier (userinfo component)
- `host` — DNS name or IP address
- `port` — QUIC listener port (default `40001`)
- `path` — Resource path (e.g. `/task/uuid-123`)

### Examples

```
adtp://a1b2c3d4@10.0.0.1:40001/transfer/uuid-abc
adtp://deadbeef@relay.example.com/task/uuid-def
```

## Relay Path

A `RelayPath` is an ordered list of `RelayHop` entries carried in the
packet context envelope:

```rust
struct RelayHop {
    peer_id: [u8; 16],
    host: String,
    port: u16,
    timestamp_ns: u64,
}

struct RelayPath {
    hops: Vec<RelayHop>,
    hop_limit: u8,
}
```

| Field | Description |
|-------|-------------|
| `hops` | Ordered list of hops the packet traversed, including origin |
| `hop_limit` | Maximum number of hops before the packet is dropped |

## Relay Protocol

### Forwarding Rules

1. **Loop Detection** — If the receiving peer's `peer_id` already appears in
   `RelayPath.hops`, the packet is dropped with `RelayLoopDetected`.
2. **Hop Limit** — If `hops.len() >= hop_limit`, the packet is dropped with
   `RelayHopLimitExceeded`.
3. **Trust Verification** — The next-hop peer ID MUST be in the local
   `TrustStore` with `TrustLevel::Trusted`.
4. **Receiver Rewrite** — The forwarding node rewrites
   `PacketHeader.receiver_peer_id` to the next hop's peer ID.
5. **Hop Append** — The forwarding node appends its own `RelayHop` before
   forwarding.

### Relay Control Flow

```
Origin                     Relay A                     Target
   |                          |                          |
   |--- adtp://target@... -->|                          |
   |                          |--- verify loop/hops --->|
   |                          |--- append hop --------->|
   |                          |--- forward packet ----->|
   |                          |                          |
   |                          |<-- ack ------------------|
   |<-- ack (via relay) -----|                          |
```

### Agent Card Verification

Before forwarding to a peer, the relay SHOULD fetch and verify the
next-hop Agent Card:
1. Fetch `http://<next-hop-host>:<agent-card-port>/.well-known/adtp-agent.json`
2. Verify the ed25519 signature using the public key in the card
3. Confirm `supports_multihop == true` if the peer is expected to relay
4. Check `relay_of` includes the target if the peer acts as a relay

## Distributed Tracing

### TraceId

A `TraceId` is a 32-byte BLAKE3 hash of a UUID v4, encoded as a 64-character
hex string. It is generated at the origin of a multi-hop relay and propagated
in the packet context envelope.

### Span Model

```rust
struct Span {
    span_id: String,
    trace_id: String,
    parent_span_id: Option<String>,
    peer_id: String,
    task_id: String,
    operation: String,
    started_at: String,       // RFC 3339
    ended_at: Option<String>, // RFC 3339
    attributes: HashMap<String, String>,
}
```

Spans are created per-hop for `relay_forward`, `relay_receive`, `task_execute`,
and `artifact_transfer` operations. A `Tracer` manages active spans and may
export completed spans via OpenTelemetry (OTLP).

## Error Codes

| Error Variant | Description |
|---------------|-------------|
| `RelayLoopDetected` | Peer ID already present in hop list |
| `RelayHopLimitExceeded` | Hop count exceeds `hop_limit` |
| `RelayForwardFailed` | Forward to next hop failed |
| `NoRelayRoute` | No trusted route to the next hop |
| `AgentCardVerificationFailed` | Agent Card signature or capability check failed |

## Security Considerations

- Agent Cards are self-signed. Peers are trusted by having their public key
  registered in the local `TrustStore`.
- Relay paths are NOT encrypted end-to-end across hops; each hop can inspect
  the packet payload. E2E encryption is a future extension.
- The Agent Card HTTP server binds to `127.0.0.1` by default and is intended
  for LAN/trusted-network deployments. Production deployments should front it
  with TLS via a reverse proxy.
