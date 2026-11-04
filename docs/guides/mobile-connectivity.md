# M.A.R.S ↔ ADTP desktop connectivity

MARS (Android, Expo) talks to ADTP Desktop over the desktop's LAN gateway.
The gateway is the long-lived listener: `adtp-api` on port **40003**, with
REST for files/telemetry and a `/relay` WebSocket for presence and live chat.

```
MARS phone ──HTTP/WS──► adtp-api :40003 ──Unix socket──► embedded agent
   │                         │
   │  register/heartbeat     ├─ paired_devices (SQLite)
   │  files, messages        ├─ device_files  (desktop → phone inbox)
   └──────────────────────── └─ device_messages (durable notes)
```

## One-time setup

### 1. Start the gateway

**Desktop app (default).** ADTP starts the gateway itself on launch, writes the
pairing token to `~/.config/adtp/api.env`, and detaches the process so it keeps
running after the window closes. Closing the window keeps ADTP in the tray (and
the embedded AI alive); **Quit** in the tray exits the desktop process, while
the gateway listener stays up.

The packaged installers ship the gateway as a **self-contained binary**, so no
Python, venv or repo checkout is needed:

- Linux `.deb`: `/usr/bin/adtp-api` (next to `adtp-desktop`).
- Windows: `adtp-api.exe` in the same folder as `adtp-desktop.exe`. Keep both
  files together, or the desktop cannot start the listener.
- Standalone `adtp-api` / `adtp-api.exe` binaries are attached to each release
  if you want to run the gateway elsewhere.

The desktop looks for the gateway in this order: `ADTP_API_BIN`, next to the
executable (`./adtp-api`, `./resources/`, `./binaries/`), the ADTP data dir,
`~/.local/bin`, `/usr/local/bin`, `/usr/bin`, then a source checkout's venv and
finally `uv run`.

**systemd user service (optional, starts at login).** This is the recommended
way to have the listener up 24/7 before the desktop app is opened:

```bash
./scripts/install-listener.sh
systemctl --user status adtp-api
journalctl --user -u adtp-api -f
```

The installer uses a bundled `adtp-api` when one is on `PATH`; otherwise it
creates a venv at `~/.local/share/adtp/venv`, installs the two Python packages,
writes the same `api.env` the desktop uses, and enables the unit. It also tries
`loginctl enable-linger` so the listener survives logout.

Open TCP 40003 on the LAN if a firewall is active:

```bash
sudo ufw allow 40003/tcp
```

### 2. Pair the phone

Use the **phone-specific** QR, which starts/reuses the gateway before the code
is rendered and always advertises the LAN address:

1. Desktop → **Settings → Paired Phones → Pair Phone (QR)** (also available in
   the Phone Listener card). The dialog shows **Listener online/offline**, the
   **Phone URL** and a firewall hint.
2. MARS → **SCAN** → scan the QR.
3. Pairing verifies `/api/v1/system/info`, registers the phone in
   `paired_devices`, and stores the token. The desktop shows the phone under
   **Settings → Paired Phones**; the phone shows the desktop on Home with live
   CPU/RAM/battery telemetry.

The generic **Pair a New Device** QR remains available for desktop-to-desktop
peering; it does not start the phone gateway.

Both sides use the same high-entropy token from `api.env` / the desktop
settings store. If the desktop reports *“Gateway running with a different
pairing token”*, stop the stale listener and reopen ADTP (or save the matching
token under Settings → Authentication).

## What works when

| Capability | Desktop app open | Window closed (tray) | Desktop process quit |
|---|---|---|---|
| Phone appears online | yes | yes | yes (gateway stays) |
| Phone → desktop files | yes | yes | yes |
| Desktop → phone files | yes | yes | yes (queued until phone polls) |
| Desktop → phone messages | yes | yes | yes (durable inbox) |
| Phone ↔ desktop AI | yes | yes | no (agent lives in desktop process) |

The AI agent is embedded in the Tauri process, so it cannot answer while ADTP
is fully quit. The gateway and file/message inbox continue to run; start ADTP
again to restore AI chat.

## Files: formats, AI attachments and chat downloads

Both directions accept arbitrary binary data. Format detection prefers the
declared `Content-Type` and falls back to the filename, so Android pickers that
report `application/octet-stream` still produce correct entries. Verified in
the live gateway E2E: **PNG, JPG, PDF, DOCX, XLSX, PPTX, ZIP, MP4, TXT, JSON,
CSV and Markdown** (hash + size + MIME + download checked both ways).
The desktop CLI also sniffs magic bytes, which covers extensionless files.

### Phone → desktop

1. Attach a file in MARS chat (paperclip) and send.
2. MARS uploads the bytes to `POST /api/v1/transfers/upload`; the gateway
   stores them in `<data_dir>/adtp/intake` and returns the absolute path.
3. The path is passed to the desktop agent as an `attachments` entry. The
   prompt includes the file name, MIME type and human-readable size, so a
   text-only model knows what arrived even when it cannot read the pixels.
4. Text files can be opened with `read_file`. Images/PDFs/archives return
   metadata instead of a UTF-8 error, so the AI can confirm and forward them.

### Desktop AI → phone

- Ask the desktop AI, for example: *“find image.json and send it to my phone”*.
- The agent uses `find_file` (searches Downloads, Documents, Pictures, Desktop,
  the upload intake and OneDrive folders by default), optionally `read_file`,
  then `send_file_to_device`.
- `send_file_to_device` stages the file in `device_files`. `device_id` is
  optional when exactly one phone is paired (or one is unambiguously online).
- The reply carries the tool-call result, and MARS renders a file tile with a
  **Download** action. Tapping it pulls
  `GET /api/v1/devices/{id}/files/{file_id}` into the app's
  `mars-inbox` document folder and removes it from the pending inbox. The same
  file is also listed on the Transfers tab.
- Delivery is durable: if the phone is offline the file waits in the inbox.

### Expanding desktop AI access

Defaults are intentionally narrow but useful for laptops:

- `ADTP_ALLOWED_PATHS` (colon-separated on Linux/macOS, semicolon on Windows)
  adds extra roots, e.g. `ADTP_ALLOWED_PATHS=/srv/files:/data/share`.
- `permissions.toml` can override `allowed_paths`/`writable_paths` explicitly.
- Hidden files stay denied; `max_file_read_bytes` still limits inline reads.

### Chat without text

An attachment sent with an empty message is still processed; the gateway
supplies a short “user sent an attachment” prompt so the AI confirms the file
and asks what to do next.

## Network and deployment compatibility

| Environment | Works? | Notes |
|---|---|---|
| Phone + Linux/Windows laptop on the same Wi-Fi | yes | Default path; QR advertises the desktop LAN IPv4 and port 40003. |
| Phone on cellular + laptop at home | yes, with a reachable address | Port-forward 40003 or use a VPN/tunnel; pair with a manual URL/hostname instead of the LAN IP. The token still protects every route. |
| EC2 / cloud VM | yes, with security-group ingress | Allow TCP 40003 from the phone's address (or a VPN range), set the desktop `api_url` to the public DNS/IP and re-pair. Prefer HTTPS via a reverse proxy when traffic crosses the internet. |
| Homelab / always-on server | yes | Install the systemd user service; it survives logout and the desktop app being closed. Files/chat inboxes are durable. AI chat needs the desktop process (the agent lives there). |
| Managed/org device (MDM, firewall, proxy) | usually, with policy changes | Outbound phone→desktop may be blocked; use the org VPN or an allowed tunnel. Local LAN discovery/mDNS may be disabled — pair with the manual IP/URL. TLS-inspecting proxies can break the WebSocket; allow `/relay` or use a direct VPN path. |
| Multiple paired phones | yes | The AI should pass `device_id`; with several online phones it refuses to guess and lists the ids. |
| IPv6-only / NAT64 | partial | Use a reachable hostname or tunnel; the QR advertises IPv4 today. |

Firewall checklist: inbound TCP 40003 on the desktop/VM, phone and desktop on
the same routable network (or VPN), and the gateway bound to `0.0.0.0` (the
desktop supervisor sets this). Health checks are unauthenticated; everything
that exposes files or device data requires the pairing token.

## Protocol notes

- `POST /api/v1/devices/register` and `.../heartbeat` keep `paired_devices`
  fresh. A live `/relay` session also touches `last_seen`, so the desktop sees
  a phone as online immediately on reconnect.
- The phone sends `deviceId` in the WebSocket `authenticate` params. The gateway
  maps that socket to the registry row and pushes desktop → phone frames to all
  live sessions.
- Phone heartbeat: REST every 20 s plus a WS `ping` every 20 s; the gateway
  answers `pong` and refreshes presence.
- Desktop → phone files are staged in `device_files` and downloaded from
  `GET /api/v1/devices/{id}/files`; messages use the matching `/messages`
  endpoints and are marked delivered by the phone.
- Desktop → phone text sent while the phone is offline is replayed by the
  phone's 20 s inbox poll and shown in the AI chat thread.

## Troubleshooting

```bash
# Gateway health (unauthenticated)
curl http://127.0.0.1:40003/api/v1/health

# Who is paired / online (requires the token)
TOKEN=$(sed -n 's/^ADTP_API_TOKEN=//p' ~/.config/adtp/api.env)
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:40003/api/v1/devices

# Is the listener bound to the LAN?
ss -ltnp | grep 40003
```

- **Phone scans but stays offline** — the phone is probably using a QR that
  advertises `127.0.0.1` (old build) or cannot reach the LAN IP. Re-open the QR
  with **Pair Phone (QR)** in the current build, check the **Phone URL** it
  shows, and allow 40003 through the firewall.
- **MARS says “Could not reach your desktop at http://…”** — that URL is the
  exact address the phone tried. If it is `127.0.0.1`/`localhost`, rebuild and
  reinstall ADTP (the QR must advertise the LAN IPv4). Otherwise check that the
  phone is on the same Wi-Fi/VPN, that the gateway is bound to `0.0.0.0`
  (`ss -ltnp | grep 40003`), and that the firewall allows TCP 40003.
- **Listener won't start** — check `~/.local/share/adtp/gateway.log`. The
  desktop tries, in order: `ADTP_API_BIN`, a sidecar next to the executable,
  the ADTP data dir / `~/.local/bin` / `/usr/bin`, `adtp-api` on `PATH`, the
  repo's `.venv`, then `uv run`. A packaged build should never reach the venv
  paths; if it reports `adtp-api unavailable: No such file or directory`,
  reinstall the latest `.deb`/`.exe` (older releases did not bundle the
  gateway) or point `ADTP_API_BIN` at the standalone binary from the release.
- **Listener starts but the phone is rejected** — the running gateway uses a
  different token. Stop it (`systemctl --user stop adtp-api`, or kill the
  process on 40003) and reopen ADTP so it rewrites `api.env`.
- **AI works on the phone only while the window is open** — expected; see the
  table above.
