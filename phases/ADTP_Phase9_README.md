# ADTP Phase 9 — A2A Transport Binding & Multi-hop Relay

## Overview

Phase 9 adds **Agent-to-Agent (A2A) Transport Binding** to ADTP, enabling
multi-hop relay, Agent Card discovery, and distributed tracing. Peers now
publish signed Agent Cards at `/.well-known/adtp-agent.json`, route packets
through intermediate relay peers with loop detection and hop-limit enforcement,
and propagate `TraceId` values across hops for end-to-end observability.

## New Files

### `crates/adtp-peer-core/src/relay.rs`
Relay path construction, loop detection, hop-limit enforcement, ADTP URI
parsing (`adtp://peer@host:port/path`), and trust verification against
the local `TrustStore`.

**Key functions:**
- `check_loop` / `check_hop_limit` — guard against forwarding loops
- `verify_trust_for_next_hop` — per-hop trust via `TrustStore`
- `build_relay_path` / `append_hop` — path construction
- `parse_adtp_uri` — ADTP URI parser
- `target_reached` — checks if last hop is the target peer

**Types:** `RelayConfig`, `RelayRequest`, `UriParts`

### `crates/adtp-peer-core/src/agent_card.rs`
Signed Agent Card creation and verification using ed25519.

**Key functions:**
- `build_unsigned_card` — builds the unsigned payload
- `sign_card` — signs with `SigningKey` (canonical JSON → ed25519)
- `verify_card` — verifies signature against embedded public key

**Types:** `AgentCard`, `UnsignedCard`

### `crates/adtp-peer-core/src/agent_card_server.rs`
Embedded HTTP server (TCP) that serves the Agent Card at
`/.well-known/adtp-agent.json`.

**Key types:**
- `AgentCardServer` — `new(port, card)`, `update_card`, `get_card_json`, `serve`

Server binds to `127.0.0.1:<port>` and runs in a `tokio::task::spawn_blocking`
loop to coexist with the tokio runtime.

### `crates/adtp-peer-core/src/trace.rs`
Distributed tracing primitives for multi-hop relay observability.

**Key types:**
- `TraceId` — 32-byte BLAKE3 hash of UUID v4, hex-encoded as 64 chars
- `Span` — span record with `span_id`, `trace_id`, `parent_span_id`, attributes
- `Tracer` — manages active spans, export via OTLP placeholder

## New Spec Documents

| Document | Path |
|----------|------|
| A2A Transport Binding | `docs/spec/a2a-transport-binding.md` |
| Agent Card Schema | `docs/spec/agent-card-schema.md` |

## Architecture — Relay Flow

```
Origin                     Relay A                     Target
   |                          |                          |
   |--- adtp://target@... -->|                          |
   |                          |--- check loop/hops ---->|
   |                          |--- append hop --------->|
   |                          |--- forward packet ----->|
   |                          |                          |
   |                          |<-- ack ------------------|
   |<-- ack (via relay) -----|                          |
```

## Agent Card Schema

```json
{
  "adtp_version": 1,
  "peer_id": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
  "public_key_hex": "e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6...",
  "display_name": "Alice's Workstation",
  "host": "10.0.0.1",
  "port": 40001,
  "max_file_size": 10737418240,
  "supported_mime_types": ["text/plain", "application/pdf"],
  "supports_tasks": true,
  "supports_multihop": true,
  "relay_of": ["deadbeef..."],
  "issued_at": "2026-06-23T12:00:00Z",
  "signature": "f1a2b3c4... (64-byte hex)"
}
```

## Relay Protocol

1. **Loop Detection** — drop if `peer_id` already in `RelayPath.hops`
2. **Hop Limit** — drop if `hops.len() >= hop_limit`
3. **Trust Verification** — next-hop must be trusted in `TrustStore`
4. **Receiver Rewrite** — forwarding node rewrites `receiver_peer_id`
5. **Hop Append** — forwarding node appends its own `RelayHop`

## CLI Changes (`adtp-peer`)

New `--agent-card-port` argument (default `40002`):

```bash
adtp-peer --listen-port 40001 --agent-card-port 40002
```

## Dependency Changes

| Dependency | Action |
|------------|--------|
| `tiny_http` | Removed from workspace (replaced by manual TCP server) |

## Test Results

```
cargo test --workspace
  adtp-core:      14 passed
  adtp-peer-core: 24 passed  (+8 relay, +2 agent_card, +3 trace)
  adtp-client:     2 passed
  adtp-server:     2 passed
  adtp-cli:       16 passed
  adtp-desktop:   11 passed
  integration:    15 passed  (+6 new Phase 9 tests)
  doc-tests:       1 passed
  -------------------------------------------------
  Total:          85/85 passed
```

### New Phase 9 Tests

| Test | Location | Covers |
|------|----------|--------|
| `test_sign_and_verify_card` | unit | Agent Card sign/verify roundtrip |
| `test_verify_tampered_card_fails` | unit | Tamper detection |
| `test_loop_detected` / `test_no_loop` | unit | Loop detection |
| `test_hop_limit_exceeded` / `test_hop_limit_ok` | unit | Hop limit |
| `test_append_hop` | unit | Path mutation |
| `test_uri_parsing*` (3) | unit | ADTP URI parser |
| `test_target_reached` | unit | Target detection |
| `test_trace_id_generation` | unit | TraceId creation |
| `test_span_lifecycle` | unit | Span start/end/attributes |
| `test_trace_id_display` | unit | TraceId hex encoding |
| `test_agent_card_sign_verify_integration` | integration | Full card lifecycle |
| `test_agent_card_server_serves_json` | integration | Card serialization |
| `test_relay_path_loop_and_hop_limit` | integration | Path integrity |
| `test_tracer_multi_hop` | integration | Cross-peer trace |
| `test_adtp_uri_parsing_integration` | integration | URI parsing |
| `test_relay_config_can_relay_for` | integration | Relay config |

## Phase 10 Hook Points

| Hook | What Phase 9 Provides | Phase 10 Uses It For |
|------|----------------------|----------------------|
| `AgentCard` relay_of field | List of peer IDs a peer relays for | Dynamic relay topology discovery |
| `TraceId` / `Span` | Distributed tracing primitives | End-to-end observability dashboards |
| `RelayPath` hop list | Ordered list of traversed hops | Path MTU negotiation, QoS routing |
| `AgentCardServer` | HTTP endpoint for `.well-known/adtp-agent.json` | Mutual TLS, federated trust bootstrap |
