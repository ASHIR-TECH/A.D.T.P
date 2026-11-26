# Hands-on ADTP / MARS test session

**Date:** 2026-11-26 (UTC)
**Focus:** ADTP desktop + gateway running locally; paired MARS phone (Redmi A5) on the LAN.
**Method:** read-only runtime audit by two parallel agents, plus live API smoke tests driven from this machine. No config, code or device state was changed except the deliberate test message/file described below.

---

## 1. Environment / runtime state

| Item | Result |
|---|---|
| Desktop process | `/usr/bin/adtp-desktop`, PID 747415, running ~1h39m |
| Gateway process | `/usr/lib/adtp/adtp-api/adtp-api`, PID/PGID 666824, running ~3h30m |
| Gateway record | `~/.local/share/adtp/gateway.json` → `{"pid":666824,"pgid":666824,"url":"http://127.0.0.1:40003"}` — matches the live process, so single-owner adoption is working |
| Auth | API token present (32 chars) in `~/.config/adtp/api.env`; used via bearer header, value never logged |
| Gateway health | `GET /api/v1/health` → **200** `{"status":"ok","version":"0.1.0","uptime_seconds":12648}` |
| Gateway log | No errors/warnings in the last 40 lines (14k+ line log) |
| Visible window | No ADTP window in the X11 client list — desktop is tray/headless or on Wayland; not treated as a failure |
| Media relay config | `ADTP_RELAY_URL` / `ADTP_MEDIA_TOKEN` are **not set** locally, so the shared Blob media path is disabled/unverified in this session |

---

## 2. Paired device

`GET /api/v1/devices` → **200**, one phone:

- Name: **Redmi A5** (`f10674c1-…-8a37`)
- Platform: android, OS 15, model `25028RN03A`, app 1.0.0
- Battery: 99%, charging
- `online: true`, `last_seen` refreshed ~16 s before the audit

The phone's IP (`192.168.1.142`) appears repeatedly in the gateway log polling `/messages` and `/files`, so the LAN relay and pairing token are functioning.

---

## 3. Live tests performed (ADTP → MARS)

### 3.1 Text message
```
POST /api/v1/devices/{id}/messages   {"text":"ADTP hands-on 16:06:59"}
→ 201, id 3fb083ca2de7452b9f6f3e4f0b92b523
```
Verification:
- `GET …/messages?include_delivered=true` → message present, durable.
- Gateway log shows the phone acknowledged it:
  `POST …/messages/3fb083ca…/delivered → 200 OK`.
- ✅ Message delivery confirmed end to end.

### 3.2 Test file
Created `/tmp/.../adtp-hands-on.txt` (67 bytes, sha256 recorded by the gateway).
```
POST /api/v1/devices/{id}/files   multipart file
→ 201, id d375372d12b246a382d237721b671057, mime text/plain
```
Verification:
- `GET …/files?include_delivered=true` → file present, `delivered_at: null`.
- Gateway log shows the phone polled `/files` after staging (GET from `192.168.1.142`).
- No `GET …/files/{file_id}` download request was observed, and MARS does not ack file delivery automatically — the phone still needs the user to tap Download in the inbox UI.
- ⚠️ Staging and visibility confirmed; actual on-phone download could not be confirmed (see blocker below).

### 3.3 Blob media path
Not exercised: `ADTP_RELAY_URL` / `ADTP_MEDIA_TOKEN` are absent from local config, so `/api/v1/media/upload` would return 503. This needs deployment config before an end-to-end blob test.

---

## 4. Blocker: MARS / ADB

A second agent audited the Android device read-only and found **no ADB device**:

```
adb devices -l          → empty
lsusb                   → Xiaomi/Redmi 2717:ff40 in MTP (Imaging) mode only
dmesg                   → repeated USB resets / error -71 on that port
```

The phone is physically connected, but USB debugging is off/unauthorized and the USB enumeration is unstable. Therefore:

- MARS package version, foreground state, logcat and inbox UI could not be inspected.
- The staged file could not be tapped/downloaded programmatically.
- No device-side change was attempted.

**To finish the on-phone half:** enable Developer Options → USB debugging, accept the RSA prompt, use a stable cable/port, then confirm `adb devices -l` shows a `device` entry. After that, re-run:
```
adb logcat -d -t 400 | grep -iE 'adtp|mars|inbox|download|media'
```
and tap Download in MARS for `adtp-hands-on.txt`.

A desktop screenshot was also attempted but X11 `GetImage` failed with `BadMatch` (likely Wayland/Xwayland); no screenshot is included.

---

## 5. Conclusion

- ADTP desktop, single-instance gateway ownership, health, pairing, live relay presence and **text message delivery** are working end to end with the paired phone.
- Desktop → phone **file staging** is working and the phone is polling the inbox; final download verification requires ADB/UI access.
- The shared Blob media path needs relay credentials configured locally before it can be tested here.
- No ADTP-side error was found in this session.
