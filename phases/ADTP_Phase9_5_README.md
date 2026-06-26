# ADTP Phase 9.5 — NAT Traversal & Universal Reachability

## Overview

Phase 9.5 ensures that **any two ADTP peers** — regardless of network topology
(laptop on home broadband, phone on cellular, EC2 instance, office LAN,
air-gapped datacenter) — can establish a QUIC connection without manual
firewall configuration, static IPs, or human intervention.

The phase adds a **connection establishment stack** below the Phase 7 Hello
handshake. The rest of ADTP is completely unaware of how the connection was
made.

## Topology Matrix

| Topology | Primary Path | Fallback |
|----------|-------------|----------|
| EC2 ↔ EC2 | Direct QUIC dial | None (already works) |
| Home ↔ EC2 (home dials out) | Direct QUIC dial | Keepalive |
| EC2 → Home (server initiates) | Rendezvous wake | Relay |
| Home ↔ Home (standard NAT) | STUN + hole punch | Relay |
| Home ↔ Home (symmetric/CGNAT) | Skip punch, relay | Relay only |
| Home ↔ Home (IPv6) | Direct IPv6 dial | IPv4 fallback |
| Same LAN | mDNS discovery | Direct dial |
| Mobile/cellular | Fast-fail to relay | Relay |
| UDP blocked (corporate) | QoWT (port 443) | None (last resort) |

## Connection Selection Algorithm

| Step | Path | Timeout | Skip Condition |
|------|------|---------|----------------|
| 1 | IPv6 direct | 500ms | No IPv6 or NAT detected |
| 2 | IPv4 direct | 500ms | No known public IP |
| 3 | mDNS LAN | 800ms | Disabled or different subnet |
| 4 | STUN + punch | 1500ms | Symmetric NAT or CGNAT |
| 5 | Relay | No limit | No trusted relay |
| 6 | QoWT | No limit | UDP not blocked |

## New Files

### `crates/adtp-rendezvous/`
Standalone Rust binary deployed alongside the public peer.

| File | Purpose |
|------|---------|
| `src/main.rs` | UDP server dispatching packet types 0x20–0x24 |
| `src/stun.rs` | STUN reflection — returns external IP:port |
| `src/token.rs` | BLAKE3-signed rendezvous tokens, single-use |
| `src/wake.rs` | Wake signal forwarding via subscriber stream |
| `src/peer_table.rs` | In-memory peer registry with TTL purging |

### `adtp-peer-core/src/` — New Modules

| File | Purpose |
|------|---------|
| `connect.rs` | Connection coordinator — runs selection algorithm |
| `nat_detect.rs` | NAT type classification via STUN |
| `paths/direct.rs` | IPv6/IPv4 direct dial |
| `paths/lan.rs` | mDNS advertisement and discovery |
| `paths/stun.rs` | STUN reflection + simultaneous UDP hole punch |
| `paths/relay.rs` | Thin wrapper around Phase 9 relay_via |
| `paths/qowt.rs` | QUIC-over-WebSocket-over-TLS (placeholder) |
| `rendezvous_client.rs` | Client side of rendezvous protocol |
| `keepalive.rs` | Ping keepalive + dead connection detection |

## New Packet Types

| Type | Hex | Purpose |
|------|-----|---------|
| `PeerRegister` | 0x20 | Peer → Rendezvous: register external addr + NAT type |
| `RendezvousRequest` | 0x21 | Peer A → Rendezvous: request punch token |
| `RendezvousResponse` | 0x22 | Rendezvous → Both: shared token + external addrs |
| `PunchReady` | 0x23 | Peer → Peer: simultaneous UDP punch |

## New Error Variants

- `NatDetectionFailed` — STUN classification failed
- `RendezvousError` — Rendezvous server communication failed
- `RendezvousTokenExpired` / `RendezvousTokenReplay` — Token validation
- `HolePunchFailed` — Simultaneous UDP punch failed
- `MdnsDiscoveryFailed` — mDNS daemon or query failed
- `QowtConnectionFailed` — WebSocket tunnel failed
- `KeepaliveTimeout` — Three missed Pongs
- `AllPathsExhausted` — All 6 paths failed

## New Config Keys (`peer.example.toml`)

| Key | Default | Purpose |
|-----|---------|---------|
| `rendezvous_server` | — | Address of the rendezvous server |
| `rendezvous_port` | 40004 | Rendezvous server UDP port |
| `lan_discovery` | true | Enable mDNS |
| `keepalive_interval_ms` | 25000 | Ping interval |
| `nat_type_override` | — | Manual NAT type override |
| `stun_servers` | google STUN | Fallback STUN servers |
| `qowt_enabled` | true | Allow QoWT fallback |
| `qowt_gateway` | — | WebSocket gateway address |
| `connection_timeout_ms` | 3000 | Per-path timeout |
| `relay_enabled` | true | Allow relay fallback |

## Test Results

```
cargo test --workspace
  adtp-core:      14 passed
  adtp-peer-core: 32 passed  (+8 new: connect, keepalive, nat_detect, stun, direct)
  adtp-client:     2 passed
  adtp-server:     2 passed
  adtp-cli:       16 passed
  adtp-desktop:   11 passed
  adtp-peer-core integration: 15 passed
  doc-tests:       1 passed
  -------------------------------------------------
  Total:          93/93 passed
```

### New Unit Tests

| Test | Module | What It Verifies |
|------|--------|-----------------|
| `test_nat_detector_default` | nat_detect | NAT type initializes to Unknown |
| `test_keepalive_initial_state` | keepalive | Missed pongs starts at 0 |
| `test_dead_detection` | keepalive | 3 missed pings = dead, pong resets |
| `test_backoff_timing` | keepalive | Exponential backoff capped at 60s |
| `test_connection_path_display` | connect | All 6 paths display correctly |
| `test_peer_config_defaults` | connect | Default config values are correct |
| `test_dial_direct_invalid_addr` | paths/direct | Direct dial to closed port fails |
| `test_stun_punch_new` | paths/stun | STUN punch socket binds |

## Phase 10 Hook Points

| Hook | What Phase 9.5 Provides | Phase 10 Action |
|------|------------------------|-----------------|
| Connection path telemetry | `adtp status` exposes active path | Aggregated path distribution as launch metric |
| Rendezvous server | Single EC2 instance, no HA | Add second rendezvous peer, test failover |
| QoWT gateway | Single gateway co-hosted | TLS from real CA, load-test concurrent WS |
| Keepalive config | Defaults tuned by trial | Tune interval for Nigerian/Kenyan ISPs |
