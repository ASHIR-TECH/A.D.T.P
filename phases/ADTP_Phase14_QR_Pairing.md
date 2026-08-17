# ADTP — Phase 14
## QR Peer Pairing
### Desktop QR Generator + MARS Mobile Scanner

**Ashir Tech | ashir.world**

---

## Overview

Phase 14 adds the fastest possible peer pairing mechanism to ADTP. Instead of copying and pasting a peer_id between devices, the user opens the Desktop App, clicks Generate QR, and the mobile phone scans it. The connection is established automatically — no manual input, no errors, no friction.

This phase has two sides that must be built together:

- **Desktop side** — a QR code generator added to the Desktop App (Phase 6/11) that encodes the full pairing payload
- **Mobile side** — a QR scanner added to MARS (Phase 14) that reads the payload, adds the peer, and initiates the first connection

Both sides are small builds. The heavy infrastructure — peer identity, trust store, rendezvous, NAT traversal — is already fully built across Phases 7 through 9.5. This phase adds a UI layer on top of it.

---

## What the QR Encodes

The QR code contains a single URI string in the ADTP deep link format:

```
adtp://pair?peer_id=a3f9bc12ee7d9f230ab1&host=192.168.1.42&port=40001&rendezvous=udp://ashir.world:40004&nickname=work-pc&pubkey=BASE64_ED25519_PUBKEY
```

### Fields

| Field | Required | Description |
|---|---|---|
| `peer_id` | Yes | The desktop peer's full hex peer_id from identity.rs |
| `host` | Yes | The desktop peer's current best-guess external IP — from the rendezvous server's last observed address |
| `port` | Yes | The QUIC port the desktop peer is listening on (default 40001) |
| `rendezvous` | Yes | The ADTP rendezvous server address — so the phone knows where to send punch requests |
| `nickname` | Yes | A human-readable name for this desktop (e.g. work-pc, home-laptop) — written to peers.toml on the mobile side |
| `pubkey` | Yes | The desktop peer's ed25519 public key, base64-encoded — used by the mobile app and the desktop REST API to verify trust without a round-trip |

### Security

The QR code is not a secret. Anyone who scans it can attempt to add the peer — but the trust model from Phase 7 still applies. The desktop peer must have `can_task_from` set to accept connections, and the pairing completes only when the desktop's trust store is updated. The QR is a convenience input method, not an authentication bypass.

For environments where the QR could be photographed by an unintended party, a short-lived mode is available: the QR displays for 60 seconds then regenerates with a new session token. The session token is an additional field (`session_token`) that the desktop REST API validates on the first connection attempt from the phone — any connection without the matching token is rejected. Session token mode is opt-in.

---

## Desktop Side — QR Generator

### Where It Lives

The QR generator is a new panel inside the Desktop App Settings screen (Phase 6/11), added as a new card below the existing Agents card. It is also accessible directly from the Peers section of the Settings screen via a **Pair New Device** button.

### New File

```
desktop/src/components/QRPairPanel.js
```

### Panel Layout

```
┌─ Pair a New Device ─────────────────────────────────────────────┐
│                                                                  │
│  Scan this QR code with the MARS mobile app to add this         │
│  PC as a trusted peer.                                          │
│                                                                  │
│          ┌─────────────────────────┐                            │
│          │                         │                            │
│          │       [QR CODE]         │                            │
│          │       240 x 240px       │                            │
│          │                         │                            │
│          └─────────────────────────┘                            │
│                                                                  │
│  Peer ID: a3f9bc12...            [Copy]                         │
│  Nickname: work-pc               [Edit]                         │
│                                                                  │
│  [Regenerate]           [Session Mode: OFF]                     │
│                                                                  │
│  Session mode expires in: --                                     │
└──────────────────────────────────────────────────────────────────┘
```

### QR Generation

The QR code is rendered as an SVG inside the Tauri webview. Use the `qrcode` npm package (pure JS, no native dependency):

```
npm install qrcode
```

Call `QRCode.toCanvas(canvas, pairingUri, { width: 240, color: { dark: '#C9A84C', light: '#0E0E1C' } })` to produce the QR in the brand gold-on-dark-purple colour scheme. The gold-on-dark-purple makes it visually distinctive and still scannable — confirmed scannable by all major camera apps at this contrast ratio.

### Data Assembly

The pairing URI is assembled by a new Tauri command:

**File:** `desktop/src-tauri/src/commands.rs` — add:

```
get_pairing_qr_data() -> Result<PairingPayload>
```

`PairingPayload` struct:

| Field | Source |
|---|---|
| `peer_id` | `P2pModule.identity.peer_id` |
| `host` | Last observed external address from `rendezvous_client.last_known_external_addr` |
| `port` | Configured QUIC port from peer.toml |
| `rendezvous` | `rendezvous_server` value from peer.toml |
| `nickname` | `display_name` from agent.toml (or machine hostname as fallback) |
| `pubkey` | `P2pModule.identity.verifying_key_bytes()` base64-encoded |
| `session_token` | Generated only when session mode is active — 32 random bytes, hex-encoded, stored in memory and expires after 60 seconds |

The frontend calls `invoke('get_pairing_qr_data')` and assembles the URI from the returned struct. The QR is regenerated every time the panel is opened, and on each Regenerate button press.

### Session Mode

When the user toggles **Session Mode ON**:

- The `session_token` field is included in the URI
- A 60-second countdown timer displays beneath the QR
- At zero seconds: the QR automatically regenerates with a fresh token and the timer resets
- The previous token is invalidated on the desktop REST API side

Session token validation is handled by a new check in the POST /api/v1/auth/oauth endpoint — if a session_token was generated, the first connection attempt from a new peer_id must include it in the pairing request body.

---

## Mobile Side — QR Scanner

### Where It Lives

The scanner is triggered by the **slide-up gesture** on the L-shaped action button on the MARS home screen (Command Center), as shown in the Figma design. It can also be accessed from the Devices screen via an **Add Device** button for users who prefer tap-over-gesture.

### New Files

```
app/scanner.tsx          — full-screen QR scanner screen
hooks/usePairing.ts      — handles the pairing flow after scan
api/pairing.ts           — API call to register the new peer
```

### Dependencies to Add

```
expo-camera              ~15.0   — camera access and QR reading
expo-barcode-scanner     ~13.0   — QR code parsing (included with expo-camera in SDK 51+)
```

Add camera permission to `app.json`:

```json
{
  "expo": {
    "plugins": [
      ["expo-camera", {
        "cameraPermission": "MARS needs camera access to scan peer QR codes."
      }]
    ]
  }
}
```

### Scanner Screen (app/scanner.tsx)

**Layout:**

```
┌──────────────────────────────────┐
│  ← Back              [Torch]     │  ← Header bar, --bg-deep at 70% opacity
│                                  │
│                                  │
│         ┌──────────┐             │
│         │          │             │  ← Animated scan frame
│         │   [QR]   │             │     Corner accents in gold
│         │          │             │     Scan line animates top → bottom
│         └──────────┘             │
│                                  │
│   Point at the QR on your PC    │  ← Offside 14px --text-primary, centred
│   screen to add it as a peer    │
│                                  │
│   Or enter Peer ID manually  →  │  ← Fallback link, --gold, bottom of screen
└──────────────────────────────────┘
```

**Camera view:** Full-screen `CameraView` from expo-camera. The scan frame is a 260×260pt square centred on screen with four gold corner accents (not a full border — just the corners, 24pt each). A gold horizontal line animates top-to-bottom in a 1.8-second loop using react-native-reanimated.

**On successful scan:** The camera freezes immediately (no flash, no sound unless the device is not on silent). A green tick animates in over the scan frame. After 400ms, the scanner closes and the pairing flow begins.

**Torch button:** Lucide `Flashlight` icon top-right. Toggles `enableTorch` on the CameraView.

**Manual fallback:** Tapping **Or enter Peer ID manually** navigates to a simple text input screen where the user can paste the full `adtp://pair?...` URI or just the peer_id separately.

### usePairing Hook (hooks/usePairing.ts)

Called immediately after a successful scan. Handles the full pairing sequence:

| Step | Action | On Failure |
|---|---|---|
| 1 — Parse URI | Parse the scanned string as a URL. Extract all fields. Validate peer_id is 40 hex chars, host is valid, port is 1–65535. | Show error: Invalid QR code — this doesn't look like an ADTP pairing code. |
| 2 — Check if already paired | Look up peer_id in the local trust store (GET /api/v1/peers — filter by peer_id). | Skip to step 4 if already trusted. |
| 3 — Register with REST API | Call POST /api/v1/peers on the desktop REST API with the pairing payload. This writes the phone's peer_id to the desktop's peers.toml as a trusted peer. | Show error: Could not reach your desktop — make sure it is online. |
| 4 — Test connection | Call GET /api/v1/health/ready. If the desktop is reachable and the new peer is registered, show success. | Show degraded success: Peer saved — connection test failed. Try again when your PC is reachable. |
| 5 — Navigate | Navigate to the Devices (Peers) tab. The new PC card appears immediately. | — |

### Pairing API (api/pairing.ts)

New typed function:

```typescript
registerPeer(payload: PairingPayload): Promise<{ success: boolean; nickname: string }>
```

Calls `POST /api/v1/peers` with body:

```json
{
  "peer_id": "a3f9bc12...",
  "host": "192.168.1.42",
  "port": 40001,
  "nickname": "work-pc",
  "pubkey": "BASE64...",
  "session_token": "optional"
}
```

---

## New REST Endpoint — Peer Registration via Mobile

One new endpoint on the Phase 4 REST API:

| Property | Detail |
|---|---|
| Method + Path | `POST /api/v1/peers` |
| Purpose | Registers a new trusted peer from a mobile pairing scan |
| Auth | Bearer ADTP API token — same as all other Phase 4 endpoints |
| Request body | `{ peer_id, host, port, nickname, pubkey, session_token? }` |
| Behaviour | Validates session_token if session mode is active. Writes a new `[[peers]]` entry to peers.toml with `trust_level = "trusted"`. Returns 201. |
| On duplicate | If peer_id already exists in peers.toml, updates the host, port and nickname. Returns 200. |
| Response | `{ nickname, peer_id, trust_level, created_at }` |
| Error — invalid session token | 401 — Session token invalid or expired |
| Error — peer_id format invalid | 422 — peer_id must be a 40-character hex string |

---

## Pairing Flow — End to End

```
Desktop App                          MARS Mobile App
──────────────────                   ──────────────────────────────

1. User opens Settings → Pair New Device
2. get_pairing_qr_data() called
3. PairingPayload assembled from
   identity, rendezvous, peer.toml
4. QR rendered on screen in gold
   on dark purple

                                     5. User slides up on L-button
                                        OR taps Add Device

                                     6. Scanner opens, camera active

                                     7. Phone camera reads QR

                                     8. URI parsed, fields validated

                                     9. POST /api/v1/peers called
                                        (to the desktop REST API)

10. Desktop writes new peers.toml
    entry — trust_level = trusted
11. Returns 201 with nickname

                                     12. GET /api/v1/health/ready called

                                     13. Success animation shown

                                     14. Navigate to Devices tab
                                         New PC card appears
```

---

## UX States

### Desktop QR Panel

| State | Visual |
|---|---|
| Loading | Spinner where QR will be — Calling rendezvous server... |
| Ready | QR displayed in gold, Peer ID shown below, Regenerate button active |
| Session mode active | Countdown timer below QR, token included in URI |
| Session expired | QR greyed out, Regenerate button pulses in gold |
| Peer just connected | Green banner above QR: work-pc connected successfully — a MARS device just paired |

### MARS Scanner

| State | Visual |
|---|---|
| Requesting permission | System permission dialog — camera permission request |
| Permission denied | Centred: camera icon with X, explanation text, Open Settings button |
| Scanning | Live camera, animated scan frame, gold corners |
| QR detected | Camera freezes, green tick over scan frame, 400ms hold |
| Parsing | Brief loading state — Connecting... |
| Success | Green banner: work-pc added. Navigating to Devices. |
| Already paired | Gold banner: work-pc is already in your peer list. Updated address. |
| Error — invalid QR | Red banner: This QR code is not a valid ADTP pairing code. |
| Error — offline | Red banner: Could not reach your desktop. Check your connection. |
| Error — session expired | Red banner: This QR code has expired. Ask the desktop to regenerate it. |

---

## File Change Summary

| File | Side | Action | Purpose |
|---|---|---|---|
| `desktop/src/components/QRPairPanel.js` | Desktop | New | QR display, session mode toggle, regenerate |
| `desktop/src/pages/Settings.js` | Desktop | Edit | Add QRPairPanel card and Pair New Device button |
| `desktop/src-tauri/src/commands.rs` | Desktop | Edit | Add `get_pairing_qr_data` Tauri command |
| `desktop/src-tauri/Cargo.toml` | Desktop | Edit | No new deps — qrcode is npm, not Rust |
| `packages/adtp-api/src/adtp_api/routers/peers.py` | Desktop API | New file | POST /api/v1/peers endpoint |
| `packages/adtp-api/src/adtp_api/models/peer.py` | Desktop API | New file | PairingRequest and PeerResponse Pydantic models |
| `packages/adtp-api/src/adtp_api/main.py` | Desktop API | Edit | Register the new peers router |
| `app/scanner.tsx` | MARS | New | Full-screen QR scanner screen |
| `hooks/usePairing.ts` | MARS | New | Pairing flow — parse, register, test, navigate |
| `api/pairing.ts` | MARS | New | Typed POST /api/v1/peers call |
| `app.json` | MARS | Edit | Add expo-camera plugin with permission string |
| `package.json` | MARS | Edit | Add expo-camera and qrcode dependencies |

---

## Testing

### Desktop — Manual QA

| Check | Expected Result |
|---|---|
| Open Pair New Device panel | QR renders within 2 seconds. Peer ID visible below. |
| QR colour scheme | Gold pattern on dark purple background. Scannable by iOS and Android camera apps. |
| Regenerate button | New QR appears immediately. Previous QR is visually replaced. |
| Session mode ON | Countdown timer starts at 60 seconds. QR changes when it reaches zero. |
| Session expired | Expired QR is greyed. Regenerate button pulses. New QR on press. |
| Peer connects after scan | Green success banner appears at top of the QR panel. |

### MARS — Manual QA

| Check | Expected Result |
|---|---|
| Slide up on L-button | Scanner opens with camera active |
| Camera permission first time | System dialog appears. App does not crash on denial. |
| Permission denied state | Error state with Open Settings link renders correctly |
| Scan a valid ADTP QR | Camera freezes, green tick, Connecting... then success banner |
| Scan an invalid QR (e.g. a URL) | Error banner: not a valid ADTP pairing code. Camera reactivates after 2 seconds. |
| Scan an expired session QR | Error banner: QR code has expired. |
| Scan a QR for an already-paired peer | Gold banner: peer already exists, address updated. |
| Devices tab after successful scan | New PC card appears with correct nickname and status |
| Manual fallback link | Navigates to text input screen. Pasting full URI works identically to scanning. |

### Unit Tests

| Test | File | Assertion |
|---|---|---|
| URI parsing — valid | hooks/usePairing.test.ts | All 6 fields extracted correctly from a valid adtp://pair URI |
| URI parsing — missing field | hooks/usePairing.test.ts | Returns ParseError with the name of the missing field |
| URI parsing — invalid peer_id | hooks/usePairing.test.ts | Returns ParseError: peer_id must be 40 hex characters |
| Session token validation | adtp_api/routers/peers.test.py | Valid token accepts. Expired token returns 401. Missing token when session mode active returns 401. |
| Duplicate peer update | adtp_api/routers/peers.test.py | Second POST with same peer_id returns 200 and updates host/port in peers.toml |

---

## Completion Checklist

- Desktop QR panel renders in Settings with correct gold-on-dark-purple QR
- QR is scannable by the iOS native camera app and the Android camera app
- Peer ID is displayed below the QR and copyable
- Regenerate button produces a new QR immediately
- Session mode counts down from 60 seconds and regenerates automatically
- `get_pairing_qr_data` Tauri command returns correct data from identity and rendezvous client
- `POST /api/v1/peers` endpoint accepts valid payload and writes to peers.toml
- `POST /api/v1/peers` rejects expired session token with 401
- MARS scanner opens from the L-button slide-up gesture
- MARS scanner opens from the Add Device button on the Devices screen
- Scan of a valid ADTP QR completes the full pairing flow
- Scan of an invalid QR shows the correct error banner
- New peer card appears on the Devices screen immediately after pairing
- Manual fallback (paste URI) works identically to scanning
- All unit tests pass

---

*Ashir Tech — ashir.world | ADTP Phase 14 — QR Peer Pairing*
