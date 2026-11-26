# Blob feature.md
# MARS ↔ ADTP Shared Media Transfer Architecture

**Document type:** Architecture review, remediation blueprint, implementation specification  
**Date:** 26 November 2026  
**Repositories reviewed:**
- MARS: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode
- ADTP: https://github.com/ASHIR-TECH/A.D.T.P

**Status:** Proposed target architecture; implementation tasks are not yet completed.  
**Scope:** Mobile-to-desktop and desktop-to-mobile media transfer, shared object storage, chat rendering, download, expiry, security, and operational verification.

---

## 1. Executive summary

MARS and ADTP should implement one shared media-transfer protocol with two platform-specific clients:

- **MARS** is the mobile client. It selects media, performs suitable on-device image optimization and thumbnail generation, obtains a short-lived upload grant, uploads directly to private object storage, finalizes the transfer, and sends a small `MediaRef` in the chat message.
- **ADTP** is the desktop client. Its Rust/Tauri desktop application and Python FastAPI gateway expose the desktop-side experience. ADTP prepares media locally, uploads it through the same shared media service, and sends the same `MediaRef` over its chat/device-message path.
- **Upstash Blob** stores the actual media objects.
- **Upstash Redis** stores short-lived transfer metadata, ownership/authorization state, expiry indexes, idempotency state, and cleanup coordination.
- **The MARS media relay** is the authoritative media control plane. ADTP authenticates to it server-to-server using `ADTP_MEDIA_TOKEN`. Neither app contains the Blob bucket token.
- **Chat transports carry metadata only.** File bytes must not be sent through WebSocket chat messages, Tauri events, agent tool-result JSON, or the ADTP/MARS chat protocol.

The desired user experience is:

1. User selects a file.
2. Sender optimizes it where safe and creates a thumbnail or preview.
3. Sender uploads the media and thumbnail to a private Blob bucket.
4. Sender sends a compact `MediaRef` in the chat message.
5. Recipient sees a media card with preview, filename, size, and a download action.
6. On download, the recipient requests a fresh short-lived read URL and saves the file locally.
7. The Blob objects and their metadata are deleted after the configured retention period (24 hours by default), including abandoned uploads.

**Important product decision:** 24-hour retention is a storage retention policy, not a guarantee that the recipient will download within 24 hours. The chat message may remain after the blob has expired; the UI must then show “Media expired” and must not pretend the file is still downloadable.

---

## 2. Repository findings

The findings below are based on source files retrieved from the two GitHub repositories. They describe the reviewed repository snapshot, not a claim that no unreviewed branch or local working-tree changes exist.

### 2.1 MARS: existing implementation

| File | Existing responsibility | Review finding |
|---|---|---|
| `mobile/src/media/compress.ts` | Image compression and thumbnail generation | Already performs on-device image conversion and thumbnail creation. It does not optimize video, audio, or documents, which is appropriate as a default. Its resize action uses a fixed width and needs orientation-aware bounds. |
| `mobile/src/media/blobClient.ts` | Sign → PUT → complete flow | Already uploads directly to Blob using signed URLs. It currently reads each file into Base64 and then creates a byte array, which can multiply memory use for large media. |
| `mobile/src/media/gatewayUpload.ts` | Multipart upload to ADTP gateway | Sends the selected source file to ADTP's `/api/v1/media/upload`; the phone-side compression pipeline is not used here. This creates a second upload path with different processing semantics. |
| `mobile/src/media/types.ts` | Shared TypeScript `MediaRef` | Useful cross-platform contract already exists. It needs explicit schema version, expiry, checksum, object size, and transfer state fields. |
| `mobile/src/media/urls.ts` | Stable media URL generation and normalization | Uses stable media routes and rewrites loopback gateway URLs for paired-device access. It should validate IDs and allowed hosts more strictly and model expired references. |
| `mobile/src/components/MediaCard.tsx` | Recipient media UI | Already displays thumbnail/file information and a Download action. `Linking.openURL` is not a consistent native “save to Downloads” implementation. |
| `mobile/relay/media.js` | Media API, authorization, signed URL minting | Has `/sign`, `/complete`, `/ingest`, `/file/:id`, metadata lookup, and delete. The metadata index is a process-local `Map` persisted to a local JSON file. There is no 24-hour expiry enforcement or scheduled Blob deletion. |
| `mobile/relay/server.js` | Relay bootstrap and media route mounting | Owns the Blob token and ADTP shared token server-side. Keep this security boundary. |
| `lib/blob.ts` | Upstash Blob SDK wrapper | Correctly uses private bucket operations and signed URLs. The current wrapper needs explicit multipart support and lifecycle/cleanup helpers. |
| `mobile/src/relay/useRelayConnection.ts` | Chat and media orchestration | Supports media references in chat. The current route prioritizes `uploadMediaViaGateway` in the send flow, so it can upload raw media to the desktop gateway instead of using the on-device compression/direct-Blob path. Verify and normalize this routing. |

Relevant source files:
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/compress.ts
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/blobClient.ts
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/gatewayUpload.ts
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/relay/media.js
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/relay/useRelayConnection.ts
- https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/docs/MEDIA_PIPELINE.md

### 2.2 ADTP: existing implementation

| File | Existing responsibility | Review finding |
|---|---|---|
| `packages/adtp-api/src/adtp_api/models/media.py` | Pydantic `MediaRef` contract | Mirrors the MARS fields and normalizes untrusted JSON. Add version, expiry, checksum, and object lifecycle state. |
| `packages/adtp-api/src/adtp_api/routers/media.py` | REST media endpoints | Already exposes `/api/v1/media/upload`, metadata lookup, authenticated file redirect, and header-free compatibility file route. The compatibility route is a bearer-capability route and needs a deliberate authorization design. |
| `packages/adtp-api/src/adtp_api/services/media_service.py` | Image preparation and relay upload | Already calls the MARS relay for sign/complete and uploads to the signed URL. It buffers the full upload in memory. It also converts images to JPEG while retaining the original MIME type/name in the upload request, which can make metadata disagree with the stored bytes. |
| `packages/adtp-api/src/adtp_api/config.py` | Gateway configuration | Already has `ADTP_RELAY_URL`, `ADTP_MEDIA_TOKEN`, and a 200 MiB media limit. |
| `packages/adtp-api/src/adtp_api/main.py` | FastAPI router registration | Media routers are registered. Preserve this central registration pattern. |
| `crates/adtp-cli/src/models.rs` | Rust wire/data models | Already contains a Rust `MediaRef` and `DeviceMessage.media`, matching the shared camelCase contract. Move the canonical type into a reusable core crate rather than letting each client drift. |
| `desktop/src/components/ChatPanel.js` | ADTP chat media rendering | Already renders a thumbnail and download link. It uses HTML string interpolation; all dynamic values, especially service URLs and filename attributes, must be safely validated/escaped. |
| `desktop/src/components/DeviceChat.js` | Per-device chat UI | Contains media-card rendering for messages sent to paired phones. Confirm that file attachments use the same `MediaRef` path rather than only the legacy local-file/device-file flow. |
| `desktop/src-tauri/src/commands.rs` | Tauri native commands | Keep filesystem writes and download destination handling on the desktop/native side; do not put storage credentials in the JS frontend. |

Relevant source files:
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/models/media.py
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/routers/media.py
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/services/media_service.py
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/config.py
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/crates/adtp-cli/src/models.rs
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/desktop/src/components/ChatPanel.js
- https://github.com/ASHIR-TECH/A.D.T.P/blob/main/desktop/src-tauri/src/commands.rs

### 2.3 Existing work that should be preserved

Do not rewrite the feature from scratch. Preserve and consolidate these existing decisions:

1. Private Upstash Blob bucket.
2. Server-side ownership of `UPSTASH_BLOB_TOKEN`.
3. Short-lived, object-scoped signed upload and read URLs.
4. Stable media IDs in chat instead of embedding bytes or expiring signed URLs.
5. One shared `MediaRef` contract.
6. MARS and ADTP compatibility through `serviceUrl`/gateway routing.
7. Image compression and thumbnail generation.
8. Chat media cards on both clients.
9. Server-to-server ADTP authentication using `ADTP_MEDIA_TOKEN`.

The target is to remove duplicated/inconsistent paths and harden lifecycle/security, not to discard working functionality.

---

## 3. Correct storage model: Blob is not Redis

Upstash offers distinct products:

- **Upstash Blob:** private S3-compatible object storage. Store original/compressed media and thumbnail bytes here.
- **Upstash Redis:** low-latency metadata and coordination store. Store transfer records, owner references, state, idempotency keys, expiry timestamps, and cleanup indexes here.

Do not place media bytes in Redis as the normal storage design. This couples large binary traffic to the metadata database, creates request-size and memory pressure, and makes lifecycle/streaming harder.

The current Upstash Redis documentation lists a 10 MB request limit for Free and Pay-as-you-go databases and a 100 MB maximum record size for those tiers. Those limits are not a suitable reason to use Redis as a media object store. Check the live plan limits before deployment:
- https://upstash.com/pricing/redis
- https://upstash.com/docs/redis/troubleshooting/max_request_size_exceeded
- https://upstash.com/docs/redis/troubleshooting/max_record_size_exceeded

Upstash Blob supports private buckets, signed URLs, and multipart upload. Its documentation describes direct upload so the server authorizes/signs the operation while the device sends bytes directly to storage. Objects over 16 MB use multipart in the documented upload-handler flow; uploads over 100 MB are recommended to use multipart/resumable handling:
- https://upstash.com/blob
- https://upstash.com/docs/blob/overall/quickstart
- https://upstash.com/docs/blob/reference/signing

**Decision:** Use the private Blob bucket named `Mars` (or a versioned bucket name if the existing bucket is not actually private). Use Redis for the index and cleanup schedule. Do not expose the Blob token to MARS or ADTP clients.

---

## 4. WhatsApp reference pattern

WhatsApp's public documentation states that media within messages is stored temporarily in encrypted form on its servers for up to 30 days to support delivery efficiency. This is not a promise that MARS/ADTP should copy WhatsApp's exact infrastructure or 30-day retention period:
- https://www.whatsapp.com/legal/privacy-policy-eea?lang=en_gb
- https://www.whatsapp.com/legal/privacy-policy-uk/revisions/20260326

A publicly circulated WhatsApp encryption whitepaper describes the classic attachment pattern: the sender encrypts media, uploads the encrypted object to a blob store, and sends the recipient a message containing a media pointer and cryptographic material; the recipient fetches and verifies/decrypts the object. Treat the whitepaper as a protocol reference, not as a complete description of WhatsApp's current internal storage infrastructure.

Meta's engineering article also documents WhatsApp's use of Rust for media consistency/security work, demonstrating the value of shared, carefully tested media parsing and validation code:
- https://engineering.fb.com/2026/01/27/security/rust-at-scale-security-whatsapp/

### 4.1 What MARS/ADTP should adopt

- Separate the **message plane** from the **media data plane**.
- Upload bytes to object storage; send only a compact reference through chat.
- Keep objects private and issue short-lived access URLs only after authorization.
- Verify integrity with SHA-256 or BLAKE3.
- Make the media reference stable while signed URLs are short-lived.
- Use a clear retention policy and asynchronous cleanup.
- Treat all uploaded media as untrusted input.

### 4.2 What MARS/ADTP must add beyond a basic WhatsApp-style pointer

For the first release, MARS/ADTP should encrypt media before upload if the product requires true end-to-end confidentiality from the storage operator. A private bucket and signed URLs provide access control, but they do **not** provide end-to-end encryption: the relay/storage provider can still process or serve plaintext objects.

Recommended E2EE v2:
1. Sender generates a random per-object data-encryption key.
2. Sender encrypts the media locally using an authenticated encryption scheme (for example, XChaCha20-Poly1305 or AES-256-GCM with unique nonce handling).
3. Sender uploads ciphertext and sends the wrapped media key only inside the authenticated/encrypted peer message.
4. Receiver downloads ciphertext, verifies the authenticated-encryption tag and digest, then decrypts locally.
5. Do not put the raw media key in Redis, Blob metadata, logs, or a public `MediaRef`.

Do not claim E2EE until the actual MARS↔ADTP message channel safely transports the media key and the recipient implementation decrypts it. Existing ADTP peer encryption and pairing are not automatically equivalent to end-to-end encryption for the separate Blob object.

---

## 5. Target system architecture

### 5.1 Logical components

```text
                 ┌───────────────────────────────┐
                 │       Shared media API        │
                 │     MARS relay / control      │
                 │                               │
                 │ Auth • grants • finalize      │
                 │ metadata • read • delete      │
                 │ expiry • audit • rate limits  │
                 └──────────────┬────────────────┘
                                │
              ┌─────────────────┴─────────────────┐
              │                                   │
       ┌──────▼──────┐                     ┌──────▼──────┐
       │ Upstash     │                     │ Upstash     │
       │ Blob        │                     │ Redis       │
       │             │                     │             │
       │ media bytes │                     │ MediaRef    │
       │ thumbnails  │                     │ owner/state │
       │ ciphertext  │                     │ TTL/index   │
       └──────▲──────┘                     └──────▲──────┘
              │                                   │
              └─────────────────┬─────────────────┘
                                │
               signed PUT / signed GET / metadata
                                │
       ┌────────────────────────┴────────────────────────┐
       │                                                 │
┌──────▼───────────┐                           ┌─────────▼─────────┐
│ MARS mobile app  │                           │ ADTP desktop app  │
│ React Native     │                           │ Tauri + Rust      │
│                  │                           │ + FastAPI gateway │
│ Pick/compress    │                           │ Pick/compress     │
│ Thumbnail        │                           │ Thumbnail         │
│ Upload           │                           │ Upload            │
│ MediaCard        │                           │ MediaCard         │
│ Save to device   │                           │ Save to computer  │
└──────────────────┘                           └───────────────────┘
```

### 5.2 Responsibilities by application

**MARS (React Native):**
- Native media picker.
- Inspect media type, file size, dimensions, and duration.
- Compress supported image formats without silently damaging unsupported formats.
- Generate a thumbnail for images; optionally generate video poster frames in a later task.
- Request upload grants from the shared media API.
- Upload directly to Blob using signed URLs.
- Finalize upload and attach `MediaRef` to `chat_message`.
- Render inbound `MediaRef` values and download to the mobile app's Downloads/share destination.
- Retry/resume uploads and clean temporary local files.

**ADTP desktop:**
- Desktop file picker.
- Stream file reads; do not load arbitrary large files entirely into memory.
- Compress supported image formats and create thumbnails.
- Use the same sign/PUT/complete contract.
- Attach `MediaRef` to `chat_response` or `device_message`.
- Render received media in the chat UI.
- Download through a native Rust/Tauri command to a user-selected destination.
- Never expose `UPSTASH_BLOB_TOKEN` or `ADTP_MEDIA_TOKEN` to the desktop JavaScript frontend.

**ADTP FastAPI gateway:**
- Authenticated desktop API surface.
- Validates local sender and media metadata.
- Provides a desktop-friendly proxy route to the shared media API.
- Does not own a separate storage bucket or a second media registry.
- Uses streaming or bounded temporary files for multipart requests.
- Converts upstream errors into stable API error responses.

**MARS relay:**
- Sole Blob token owner.
- Validates MARS user JWT or ADTP service token.
- Creates media IDs and object paths.
- Creates signed upload/read grants.
- Verifies uploaded objects before marking a transfer complete.
- Persists metadata in Redis.
- Enforces object access policy.
- Deletes objects and metadata after expiry.
- Emits structured audit/metrics events.

---

## 6. Canonical transfer lifecycle

### 6.1 MARS → ADTP

```text
User taps Attach
   |
   v
React Native picker returns local URI
   |
   v
Validate type / size / dimensions
   |
   v
Compress image where safe + generate thumbnail
   |
   v
POST /api/v1/media/uploads
   |
   v
Relay authorizes sender and creates pending Redis record
   |
   v
Relay returns signed PUT URL(s) + exact required headers
   |
   v
Phone PUTs bytes directly to private Upstash Blob
   |
   v
Phone POSTs /complete
   |
   v
Relay verifies object metadata, size, checksum and owner
   |
   v
Relay marks transfer complete and returns MediaRef
   |
   v
MARS sends chat_message { text, media: MediaRef }
   |
   v
ADTP receives metadata only and renders media card
   |
   v
User clicks Download
   |
   v
ADTP asks media API for authorized download URL
   |
   v
Rust/Tauri saves file to selected local path
```

### 6.2 ADTP → MARS

```text
ADTP user / agent selects or creates media
   |
   v
ADTP validates and prepares media
   |
   v
ADTP FastAPI media_service calls MARS relay /sign
   |  Authorization: Bearer ADTP_MEDIA_TOKEN
   v
Relay creates pending Redis record and signed PUT URL(s)
   |
   v
ADTP uploads bytes directly to Blob using signed URL(s)
   |
   v
ADTP calls relay /complete
   |
   v
Relay verifies and returns MediaRef
   |
   v
ADTP relay sends chat_response.media or device_message.media
   |
   v
MARS receives MediaRef and renders MediaCard
   |
   v
User taps Download
   |
   v
MARS requests fresh download URL and saves/shares file locally
```

### 6.3 Transfer state machine

Use the following explicit states:

| State | Meaning | Allowed next states |
|---|---|---|
| `created` | Metadata record and upload intent created | `uploading`, `cancelled`, `expired`, `failed` |
| `uploading` | At least one object upload is in progress | `verifying`, `failed`, `cancelled`, `expired` |
| `verifying` | All expected objects have uploaded; server verifies them | `ready`, `failed`, `expired` |
| `ready` | MediaRef may be attached to chat and downloaded | `deleting`, `expired` |
| `failed` | Upload or validation failed | `created` only through a new idempotent attempt, or `deleting` |
| `cancelled` | Sender cancelled | `deleting` |
| `deleting` | Cleanup worker is deleting objects | `expired` |
| `expired` | Retention deadline passed; object is unavailable | terminal |
| `deleted` | Blob objects and metadata removed | terminal |

Do not mark a transfer `ready` simply because the client called `/complete`. The server must verify the actual uploaded objects.

---

## 7. Canonical data contract

### 7.1 MediaRef v2

Keep the existing camelCase naming to avoid breaking current clients. Add a schema version and explicit lifecycle fields.

```json
{
  "schemaVersion": 2,
  "id": "0194f3c2-...",
  "serviceUrl": "https://media.example.com",
  "name": "design-review.jpg",
  "mimeType": "image/jpeg",
  "size": 8400000,
  "storedSize": 410000,
  "width": 1600,
  "height": 1200,
  "durationMs": null,
  "sha256": "hex-encoded-sha256",
  "sender": "mars-user",
  "recipient": "adtp-desktop-id",
  "conversationId": "default-session",
  "compressed": true,
  "hasThumbnail": true,
  "createdAt": "2026-11-26T10:00:00.000Z",
  "expiresAt": "2026-11-27T10:00:00.000Z",
  "status": "ready"
}
```

Field rules:
- `id`: random UUIDv4/UUIDv7 or at least 128 bits of cryptographically secure randomness. Never sequential.
- `serviceUrl`: trusted API base URL; never accept arbitrary schemes or untrusted host overrides.
- `name`: display name only; sanitize before using in `Content-Disposition` or local filesystem paths.
- `mimeType`: server-detected/validated MIME type, not blindly trusted from picker metadata.
- `size`: original byte length.
- `storedSize`: actual bytes stored for the primary object.
- `sha256`: checksum of stored bytes; for E2EE, checksum ciphertext and optionally include a separate authenticated plaintext digest only if the threat model permits it.
- `recipient` and `conversationId`: required for authorization. They must be assigned by authenticated server context, not trusted from a client body.
- `expiresAt`: server-calculated absolute UTC timestamp.
- `status`: `ready` only after server-side verification.

The old `compressedSize` field can remain as a deprecated alias for `storedSize` for one compatibility release. Do not remove it until both clients and all persisted message readers are migrated.

### 7.2 Chat message payload

MARS → ADTP:

```json
{
  "type": "chat_message",
  "sessionId": "default-session",
  "text": "Please inspect this image.",
  "media": {
    "schemaVersion": 2,
    "id": "0194f3c2-...",
    "serviceUrl": "https://media.example.com",
    "name": "design-review.jpg",
    "mimeType": "image/jpeg",
    "size": 8400000,
    "storedSize": 410000,
    "sha256": "hex-encoded-sha256",
    "hasThumbnail": true,
    "expiresAt": "2026-11-27T10:00:00.000Z",
    "status": "ready"
  }
}
```

ADTP → MARS:

```json
{
  "type": "chat_response",
  "sessionId": "default-session",
  "text": "I have attached the generated preview.",
  "timestamp": "2026-11-26T10:02:00.000Z",
  "media": {
    "schemaVersion": 2,
    "id": "0194f3c2-...",
    "serviceUrl": "https://media.example.com",
    "name": "generated-preview.png",
    "mimeType": "image/png",
    "size": 2100000,
    "storedSize": 280000,
    "sha256": "hex-encoded-sha256",
    "hasThumbnail": true,
    "expiresAt": "2026-11-27T10:02:00.000Z",
    "status": "ready"
  }
}
```

The chat transport must never contain:
- Base64 media payloads.
- Raw Blob URLs.
- Blob bucket tokens.
- Signed upload URLs.
- Signed download URLs.
- Filesystem paths from the sender's machine.
- Encryption keys in plaintext.

### 7.3 Redis record model

Use namespaced keys. Suggested schema:

```text
media:{mediaId}                 HASH/JSON  authoritative metadata
media:expiry                    ZSET       score = expiresAt epoch seconds
media:owner:{ownerId}           SET        media IDs owned/sent by owner
media:conversation:{convId}     SET        media IDs associated with conversation
media:upload:{mediaId}          HASH       upload state, expected object keys, grant expiry
media:idempotency:{owner}:{key} STRING     media ID, with short TTL
media:cleanup:lock              STRING     distributed cleanup lock
```

Example metadata:

```json
{
  "id": "0194f3c2-...",
  "ownerId": "user-or-device-id",
  "senderType": "mars_user",
  "recipientId": "adtp_device_id",
  "conversationId": "default-session",
  "objectKey": "media/2026/11/26/0194f3c2/original.jpg",
  "thumbnailKey": "media/2026/11/26/0194f3c2/thumbnail.jpg",
  "mimeType": "image/jpeg",
  "name": "design-review.jpg",
  "originalSize": 8400000,
  "storedSize": 410000,
  "sha256": "hex",
  "status": "ready",
  "createdAt": "2026-11-26T10:00:00.000Z",
  "expiresAt": "2026-11-27T10:00:00.000Z"
}
```

Use Redis TTL on the metadata record as a secondary safeguard, but **do not rely on Redis key expiry to delete Blob objects**. Redis expiry removes Redis keys; it does not automatically delete an object in Blob. The sorted-set cleanup worker must delete the Blob objects first and then remove the metadata keys.

Use atomic operations/Lua scripts or a transaction where state transitions and expiry-index updates must happen together. Make all operations idempotent.

---

## 8. API specification

All endpoints below are under the media API base. Existing routes can be retained as compatibility aliases while clients migrate.

### 8.1 Authentication

**MARS user request**
```http
Authorization: Bearer <Supabase access token>
```

The relay must verify the JWT with Supabase and derive the user ID from the verified claims.

**ADTP service request**
```http
Authorization: Bearer <ADTP_MEDIA_TOKEN>
```

This token is server-to-server only. It must be stored in ADTP gateway configuration and MARS relay environment variables. It must never be sent to the desktop JavaScript bundle, mobile bundle, logs, chat payloads, or signed URL query strings.

Recommended follow-up: replace one shared static ADTP token with per-desktop credentials or short-lived service tokens, scoped to upload/read/delete operations and rotated independently.

### 8.2 `POST /api/v1/media/uploads`

Create an upload intent and return signed upload grants.

Request:
```http
POST /api/v1/media/uploads
Content-Type: application/json
Authorization: Bearer <user-or-service-token>
Idempotency-Key: <random-client-generated-key>
```

```json
{
  "name": "design-review.jpg",
  "mimeType": "image/jpeg",
  "originalSize": 8400000,
  "storedSize": 410000,
  "width": 1600,
  "height": 1200,
  "compressed": true,
  "thumbnail": {
    "mimeType": "image/jpeg",
    "size": 28000,
    "width": 320,
    "height": 240
  },
  "conversationId": "default-session",
  "recipientId": "adtp-device-id"
}
```

Response `201 Created`:
```json
{
  "id": "0194f3c2-...",
  "status": "created",
  "expiresAt": "2026-11-27T10:00:00.000Z",
  "uploadExpiresAt": "2026-11-26T10:10:00.000Z",
  "objects": {
    "original": {
      "objectKey": "media/2026/11/26/0194f3c2/original.jpg",
      "method": "PUT",
      "url": "https://signed-upload-url",
      "headers": {
        "content-type": "image/jpeg",
        "content-length": "410000"
      }
    },
    "thumbnail": {
      "objectKey": "media/2026/11/26/0194f3c2/thumbnail.jpg",
      "method": "PUT",
      "url": "https://signed-thumbnail-upload-url",
      "headers": {
        "content-type": "image/jpeg"
      }
    }
  }
}
```

Requirements:
- Generate object keys on the server. Never accept an arbitrary object key from the client.
- Enforce maximum original and compressed sizes per MIME/category.
- Validate image dimensions and thumbnail dimensions.
- Bind signed PUT grants to object path, method, content type, exact size when supported, and a short expiry.
- Grant expiry should be 10 minutes or less.
- Save `created` metadata and expiry index before returning grants.
- Return exact headers; clients must send them unchanged.
- Apply rate limits per user/device and per IP.
- Idempotency key retries must return the same pending upload record/grants where still valid, or a fresh grant for the same media ID.

### 8.3 `POST /api/v1/media/uploads/{id}/complete`

Finalize upload.

Request:
```http
POST /api/v1/media/uploads/0194f3c2-.../complete
Authorization: Bearer <user-or-service-token>
Content-Type: application/json
```

```json
{
  "objects": {
    "original": {
      "size": 410000,
      "sha256": "hex-encoded-sha256"
    },
    "thumbnail": {
      "size": 28000,
      "sha256": "hex-encoded-sha256"
    }
  }
}
```

Response `200 OK`:
```json
{
  "media": {
    "schemaVersion": 2,
    "id": "0194f3c2-...",
    "serviceUrl": "https://media.example.com",
    "name": "design-review.jpg",
    "mimeType": "image/jpeg",
    "size": 8400000,
    "storedSize": 410000,
    "sha256": "hex-encoded-sha256",
    "hasThumbnail": true,
    "createdAt": "2026-11-26T10:00:00.000Z",
    "expiresAt": "2026-11-27T10:00:00.000Z",
    "status": "ready"
  }
}
```

Requirements:
- Verify caller owns this pending upload.
- Verify every expected object exists.
- Verify actual object size and content type against the grant.
- Verify checksum if the Blob API/SDK provides a trusted checksum or compute it server-side where feasible. Do not trust a client-supplied checksum as proof.
- Verify thumbnail presence and dimensions.
- Reject missing or mismatched objects; transition to `failed` and schedule cleanup.
- Set `ready` only after all checks pass.
- Do not extend the 24-hour expiry on repeated `/complete` calls.
- Return the same MediaRef on idempotent repeated completion.

### 8.4 `GET /api/v1/media/{id}`

Return metadata only after authorization.

Response:
```json
{
  "media": {
    "schemaVersion": 2,
    "id": "0194f3c2-...",
    "name": "design-review.jpg",
    "mimeType": "image/jpeg",
    "size": 8400000,
    "storedSize": 410000,
    "hasThumbnail": true,
    "createdAt": "2026-11-26T10:00:00.000Z",
    "expiresAt": "2026-11-27T10:00:00.000Z",
    "status": "ready"
  }
}
```

Return:
- `404` for unknown IDs.
- `410 Gone` for expired/deleted media.
- `403` when caller is not the sender, recipient, or authorized service.
- Never reveal object keys or storage credentials.

### 8.5 `POST /api/v1/media/{id}/download-url`

Authorize the recipient and mint a fresh signed GET URL.

Request:
```json
{
  "kind": "original"
}
```

`kind` is `original` or `thumbnail`.

Response:
```json
{
  "url": "https://short-lived-signed-read-url",
  "expiresAt": "2026-11-26T10:05:00.000Z",
  "filename": "design-review.jpg",
  "mimeType": "image/jpeg",
  "size": 410000
}
```

Requirements:
- Check recipient/device/conversation authorization before signing.
- Signed URL expiry should be short (for example 2–5 minutes; Upstash Blob signed reads are capped at 10 minutes).
- Set the correct `Content-Disposition` for downloads and safe filename.
- Set private/no-store caching for sensitive objects.
- Do not store signed URLs in chat history.
- The UI requests a new URL each time the user clicks Download.

### 8.6 `DELETE /api/v1/media/{id}`

Authenticated owner cancellation/deletion.

Response:
```json
{
  "id": "0194f3c2-...",
  "status": "deleting"
}
```

Requirements:
- Only sender/owner or authorized administrator can delete early.
- Delete original and thumbnail objects.
- Remove Redis metadata/index references after object deletion succeeds.
- Retry partial deletion safely.
- Return `204` or a stable deletion response for repeated requests.

### 8.7 `GET /api/v1/media/{id}/status`

Return lifecycle state and expiry:
```json
{
  "id": "0194f3c2-...",
  "status": "ready",
  "expiresAt": "2026-11-27T10:00:00.000Z"
}
```

### 8.8 Compatibility endpoints

Keep existing paths temporarily:

| Existing path | Target canonical path | Action |
|---|---|---|
| `POST /api/media/sign` | `POST /api/v1/media/uploads` | Retain as adapter; map old fields to v2 |
| `POST /api/media/complete` | `POST /api/v1/media/uploads/{id}/complete` | Retain as adapter |
| `GET /api/media/file/{id}` | `POST /api/v1/media/{id}/download-url` | Keep only as a compatibility redirect after authorization |
| `GET /api/media/{id}` | `GET /api/v1/media/{id}` | Retain adapter |
| `DELETE /api/media/{id}` | `DELETE /api/v1/media/{id}` | Retain adapter |

The existing header-free `/api/media/file/{id}` route is effectively a bearer-capability URL because it does not require authorization. For the target design, prefer authenticated download-url requests. If the product deliberately chooses capability links, use high-entropy random IDs, make them recipient-scoped, enforce expiry at the metadata layer, and document that anyone possessing the link can access the file until expiry. Do not accidentally treat an opaque ID as both a public credential and a normal database ID.

---

## 9. Compression and thumbnail policy

### 9.1 Images

MARS:
- Use `expo-image-manipulator` or the current supported Expo API.
- Resize using the actual aspect ratio and ensure `max(width, height) <= 1600`.
- Generate a thumbnail with `max(width, height) <= 320`.
- Encode JPEG only when the input is a photographic image and conversion is acceptable.
- Preserve PNG transparency when required.
- Preserve animated GIF/WebP or explicitly disclose that animation is flattened. Do not silently convert animated media to a still image.
- Keep EXIF orientation handling consistent.
- Clean up intermediate files after upload completion/failure.

ADTP:
- Use Pillow for image operations where supported.
- Use the same maximum-edge and quality policy as MARS.
- Keep the output filename extension and MIME type aligned with the actual encoded format.
- Use a streaming/bounded temporary-file strategy for large images.
- Limit pixel count and decoded image memory to mitigate decompression bombs.

**Existing ADTP bug to fix:** `media_service.prepare()` converts images to JPEG, but `store()` currently passes the original `mime_type` and original `name` to the relay. For example, a PNG may be stored as JPEG while its `MediaRef` still says `image/png` and may retain a `.png` filename. Return the prepared output MIME type and filename from `prepare()` and use those values for signing, storage, and the MediaRef.

**Existing MARS issue to fix:** `compress.ts` uses a fixed resize width. For portrait images, a width-only resize can violate the intended maximum longest-edge constraint. Calculate the correct target width/height based on orientation, or use the current Expo API's max-width/max-height options.

### 9.2 Video

- Do not blindly recompress video in the first release. Transcoding is CPU-intensive and can increase upload time, battery use, and thermal load.
- Preserve the original video by default.
- Generate a poster frame using a native video frame extractor if available.
- Record duration, width, height, codec/container where known.
- Enforce a separate video size limit and upload through multipart/resumable Blob APIs.
- Add server-side malware/media scanning where the threat model requires it.

### 9.3 Audio

- Preserve the original audio format by default.
- Record duration, codec, bitrate, and sample rate when available.
- Generate waveform/cover art only as a later enhancement.
- Do not transcode voice recordings without an explicit product requirement.

### 9.4 Documents and arbitrary files

- Never lossy-compress documents.
- Preserve exact bytes and filename (after safe normalization).
- Generate a generic file-type tile or PDF first-page preview only when supported and sandboxed.
- Never render uploaded HTML/SVG as trusted active content in the desktop WebView.
- Validate file signatures (magic bytes) in addition to declared MIME type.

### 9.5 Memory and streaming

Current MARS `blobClient.ts` reads file bytes as Base64, converts them to bytes, and then uploads. This creates avoidable memory copies. Replace this with a file-upload API that streams a local URI when the React Native runtime/platform supports it, or use a native upload task with progress/cancellation. Do not use Base64 for large files.

Current ADTP `media_service.store_upload()` calls `await file.read()` and holds the full object in memory before compression/upload. Replace with:
- bounded streaming to a temporary file;
- streaming hash calculation;
- size enforcement while reading;
- image processing from a bounded file path;
- streaming/multipart upload for large files;
- cleanup of temporary files in `finally`.

Do not read unbounded user-controlled media into RAM.

---

## 10. 24-hour retention and cleanup architecture

### 10.1 Retention rule

Default policy:
- `expiresAt = createdAt + 24 hours`.
- Upload grants expire much sooner (10 minutes or less).
- Signed download links expire in 2–5 minutes (never more than Upstash's 10-minute maximum).
- The original and thumbnail share one retention deadline.
- Repeated downloads do not extend retention.
- Re-sending the same media reference does not silently reset retention.
- Explicit re-upload creates a new media ID and new retention window.

### 10.2 Cleanup worker

Run a scheduled cleanup worker every 5–15 minutes.

Pseudo-flow:
```text
Acquire distributed lock
  |
  v
Read media:expiry entries with score <= now
  |
  v
For each media ID:
  Load metadata
  |
  +-- no metadata --> remove stale expiry index entry
  |
  +-- not expired --> repair index / skip
  |
  +-- expired:
        mark deleting
        delete original + thumbnail from Blob
        if delete succeeded:
            delete metadata and owner/conversation indexes
            mark expired/deleted where tombstone policy requires it
        else:
            record retry count + last error
            leave for next worker run
  |
  v
Release lock
```

Requirements:
- Use a distributed lock with a short lease and unique lock token.
- Use a compare-and-delete/release operation so one worker cannot release another worker's lock.
- Process bounded batches.
- Use exponential backoff for failed object deletion.
- Emit metrics for deleted objects, expired records, retries, and cleanup lag.
- Alert if cleanup lag exceeds 30 minutes.
- Ensure partial deletion is idempotent.
- Run a daily reconciliation job that compares expired Redis records and Blob objects by the `media/` prefix.
- Handle stale multipart uploads separately; abort unfinished multipart uploads older than one day using the Blob SDK's supported API.

**Important:** Do not assume Redis TTL deletes Blob data. Redis key expiration and Blob object deletion are separate operations.

### 10.3 Expired message behavior

A message can outlive its media object. When a recipient opens an old message:
- `GET metadata` returns `410 Gone` or `status: expired`.
- The media card changes to an “Expired” state.
- Hide or disable the Download action.
- Keep the message text and non-sensitive metadata if chat history retention allows it.
- Offer “Ask sender to resend” only if the product wants that interaction.

### 10.4 Redis retention keys

Recommended:
- `media:{id}` gets TTL slightly longer than the object retention deadline to preserve a short tombstone window.
- `media:expiry` sorted set stores exact deletion deadlines.
- `media:upload:{id}` expires at upload-grant deadline plus a small retry grace period if not completed.
- `media:idempotency:{owner}:{key}` expires after a few hours.
- Owner/conversation indexes are removed when media is deleted.

Do not use a single `EXPIRE` on a hash and assume that a worker will still know which Blob keys to delete. The expiry sorted set is the durable cleanup queue.

---

## 11. Security requirements

### 11.1 Secrets

- `UPSTASH_BLOB_TOKEN`: MARS relay only.
- `ADTP_MEDIA_TOKEN`: MARS relay + ADTP FastAPI server only.
- Supabase service-role credentials: server only.
- No secret in Expo `EXPO_PUBLIC_*` variables.
- No secret in Tauri frontend JS, app bundle, source-controlled config, crash logs, or telemetry.
- Rotate compromised tokens immediately.

### 11.2 Authorization

Current MARS relay supports Supabase JWT and `ADTP_MEDIA_TOKEN`. Keep that distinction, but strengthen the model:

- Resolve a stable `ownerId` from verified JWT claims or registered ADTP device identity.
- Never use the hash of a bearer token as the long-lived user identity.
- Bind each upload to sender, recipient/device, and conversation.
- Authorize every metadata lookup and download-url request.
- Authorize early deletion by owner.
- Do not allow an arbitrary `serviceUrl` from a message to become a trusted fetch target.
- Enforce an allowlist of media API hosts for MARS and ADTP.
- Never let the API accept arbitrary Blob object paths from clients.

### 11.3 MIME, file and path safety

- Validate declared MIME against file signature.
- Normalize filename; reject path separators, control characters, null bytes, CR/LF, and traversal.
- Store using server-generated object keys.
- Use safe `Content-Disposition` generation.
- Do not trust client-provided file size or dimensions.
- Enforce size limits while streaming, not only after reading the body.
- Limit image pixel count and decompression ratio.
- Do not automatically open or execute downloaded files.
- Do not render arbitrary HTML/SVG content inline in Tauri WebView.
- Scan executable/script/document formats if required by deployment policy.

### 11.4 Signed URLs

- Signed upload URLs must be scoped to one object, one method, exact headers, and a short lifetime.
- Signed read URLs must be minted only after authorization.
- Never persist signed URLs in chat messages.
- Never log full signed URLs because their query strings are bearer credentials.
- Do not cache sensitive signed-read responses.
- Follow Upstash's returned `expiresAt`, not a locally guessed expiry.
- Upstash documents a maximum 10-minute signed URL lifetime and requires clients to send signed upload headers exactly as returned:
  https://upstash.com/docs/blob/reference/signing

### 11.5 Access policy

The existing header-free `/api/media/file/{id}` endpoint is convenient for `<Image>` tags, but it means the ID behaves as a bearer capability. This is acceptable only if explicitly designed and bounded.

Preferred v2:
- MARS and ADTP request a download URL through an authenticated API call.
- The API checks that the caller is a participant in the transfer/conversation.
- The API returns a short-lived signed URL.
- The client downloads from Blob.
- For React Native image rendering, either use a short-lived authenticated thumbnail URL or fetch the thumbnail through an authenticated native request and cache it locally with a bounded cache policy.

If a header-free stable image URL is retained for React Native `<Image>`, make it a deliberate capability endpoint with:
- cryptographically random ID;
- recipient-scoped media record;
- strict expiry;
- rate limiting;
- no listing;
- no metadata disclosure;
- no permanent URL;
- explicit acceptance that possession of the URL grants access until expiry.

---

## 12. React Native implementation structure

Recommended target organization (adapt names to the existing tree rather than moving everything in one PR):

```text
mobile/src/
  media/
    api/
      mediaApi.ts              # sign, complete, metadata, download-url, delete
      mediaTransport.ts        # signed PUT / native upload task / progress
    domain/
      mediaTypes.ts            # MediaRefV2, MediaSource, MediaState
      mediaValidation.ts       # MIME, size, metadata validation
      mediaPolicy.ts           # image/video/document policies
    processing/
      imageProcessor.ts        # resize, re-encode, thumbnail
      videoPoster.ts           # optional poster frame extractor
    hooks/
      useMediaUpload.ts        # upload lifecycle + cancellation
    components/
      MediaCard.tsx            # preview, status, download
    storage/
      localMediaCache.ts       # bounded thumbnail cache
      mediaDownload.ts         # save/share to local device
```

### 12.1 Upload sequence in MARS

1. Picker returns URI, name, MIME, and size.
2. Validate size before decoding.
3. Determine file category.
4. For images, compress and create thumbnail locally.
5. Calculate checksum over the final stored bytes.
6. Call `POST /api/v1/media/uploads`.
7. PUT the compressed file and thumbnail using exact signed headers.
8. Report progress from actual uploaded bytes; do not simulate 100%.
9. Call `/complete`.
10. Validate the returned `MediaRef`.
11. Add the `MediaRef` to the optimistic chat message.
12. Send the chat message.
13. If the chat send fails after successful upload, retain the upload as an unsent draft for a short bounded period or delete it. Do not leave orphaned Blob objects until the full 24 hours if the client knows the send failed.
14. Remove local intermediate compression/thumbnail files.

### 12.2 MARS download sequence

1. User taps Download.
2. Request authorized download URL for `kind=original`.
3. Use native file download task to save into app cache or Downloads/share flow.
4. Verify final file size/checksum.
5. Move/copy to user-selected destination if supported.
6. Surface progress, cancel, retry, and failure states.
7. Never download the full original merely to render a thumbnail.

### 12.3 MARS corrections to current implementation

- Make direct-to-Blob upload after on-device compression the primary path.
- Make `uploadMediaViaGateway` a compatibility fallback only, or refactor it to accept already-compressed local artifacts and call the canonical media API rather than creating a second upload workflow.
- Remove Base64 whole-file conversion for large files.
- Correct portrait/landscape resize dimensions.
- Keep animated and transparent formats safe.
- Add cancellation and retry with idempotency.
- Add progress based on actual bytes transferred.
- Replace `Linking.openURL`-only download behavior with a native save/share workflow.
- Add `expiresAt` handling to `MediaCard`.
- Harden `normalizeMediaRef` with schema validation, non-negative size checks, valid UUID constraints, MIME bounds, URL host allowlist, and expiry parsing.

---

## 13. ADTP implementation structure

ADTP is not simply a Rust binary with a React UI. The reviewed repository includes:
- Rust workspace crates for core protocol, peer transfer, CLI, and Tauri native integration.
- A Tauri v2 desktop application with a JavaScript frontend.
- A Python FastAPI gateway under `packages/adtp-api`.
- Agent and relay integration through that gateway.

The media implementation should respect those boundaries.

Recommended target organization:

```text
crates/
  adtp-core/
    src/
      media/
        mod.rs
        types.rs              # canonical Rust MediaRef
        validation.rs         # shared field and MIME validation
        integrity.rs          # checksum helpers

packages/adtp-api/src/adtp_api/
  models/
    media.py                  # Pydantic API model
  routers/
    media.py                  # REST route adapters
  services/
    media_service.py          # relay client, streaming, lifecycle
    media_processing.py       # image metadata/thumbnail policy

desktop/
  src/
    components/
      MediaCard.js            # safe renderer
    api/
      media.js                # frontend calls to Tauri/API, no secrets
  src-tauri/
    src/
      commands/
        media.rs              # native download/save commands
```

### 13.1 Rust responsibilities

- Define the canonical serializable `MediaRef` in a reusable core crate (likely `adtp-core` or a dedicated shared wire-contract crate).
- Keep serde camelCase serialization explicit and covered by fixture tests.
- Validate IDs, URLs, MIME strings, file sizes, expiry timestamps, and status values at trust boundaries.
- Use async file/network I/O for network-bound operations; use bounded buffers and streaming.
- Keep CPU-heavy image processing off the async executor thread. Use a dedicated blocking worker or spawn-blocking boundary, or use a controlled image-processing worker.
- Keep native filesystem writes inside Rust/Tauri commands with least-privilege path selection.
- Never hold the Tokio runtime worker while performing a long synchronous file read or CPU-bound image operation.

Tokio documents `AsyncRead` and `AsyncWrite` for asynchronous streaming I/O. Its tutorial also cautions that async does not automatically make CPU-bound work faster; CPU-heavy processing should be handled separately:
- https://tokio.rs/tokio/tutorial/io
- https://tokio.rs/tokio/tutorial

Cargo workspaces are the appropriate mechanism for shared Rust types and dependency policy across ADTP crates:
- https://doc.rust-lang.org/cargo/reference/workspaces.html

### 13.2 FastAPI responsibilities

- Keep `/api/v1/media/*` as the desktop-facing REST adapter.
- Authenticate the desktop API caller with the existing ADTP API token/pairing identity.
- Do not expose `ADTP_MEDIA_TOKEN` to the Tauri frontend.
- Stream upload data to bounded temporary storage rather than calling `await file.read()` without a bound.
- Compress and create thumbnails with the same policy as MARS.
- Call the shared MARS relay for sign/complete/download authorization.
- Do not create a second independent media index in the ADTP database.
- Return a `MediaRef` whose service URL can be reached by the paired MARS device. Do not return an unconditionally loopback-only URL to a phone.

### 13.3 Tauri desktop responsibilities

- Use Tauri commands for save/download operations.
- Let the user choose a destination directory or use an explicit app-managed Downloads folder.
- Write to a temporary `.part` file, verify the completed file, then atomically rename it.
- Do not construct local file paths from untrusted `MediaRef.name`.
- Keep filesystem permissions narrowly scoped in Tauri capabilities.
- The JavaScript frontend should request a download operation; Rust owns the actual file write and integrity check.

Tauri's filesystem documentation recommends native `std::fs`/`tokio::fs` for Rust-side file operations and requires explicit capability scopes for frontend filesystem access:
- https://v2.tauri.app/plugin/file-system/

### 13.4 ADTP corrections to current implementation

- Fix MIME/filename mismatch after Pillow converts images to JPEG.
- Replace full-file memory buffering in `media_service.store_upload`.
- Add actual object verification before `/complete` becomes `ready`.
- Add explicit expiry and cleanup state.
- Ensure ADTP desktop UI download requests are authorized and use fresh signed URLs.
- Harden HTML rendering in `ChatPanel.js` and `DeviceChat.js`.
- Confirm that all desktop-originated media paths (agent output, user-selected attachment, direct device chat, and queued device message) produce the same `MediaRef` shape.
- Consolidate duplicate `MediaRef` definitions into a single Rust canonical type plus generated/fixture-validated TypeScript and Pydantic counterparts.

---

## 14. Frontend rendering and download behavior

### 14.1 Media card states

Every media card must support:

| State | UI |
|---|---|
| `uploading` | Thumbnail/local preview, progress bar, cancel |
| `processing` | “Preparing preview…” |
| `ready` | Thumbnail or file icon, name, size, Download |
| `downloading` | Download progress, cancel |
| `downloaded` | Open/Show in folder action |
| `failed` | Retry or remove |
| `expired` | “Media expired” and disabled download |
| `deleted` | “Media unavailable” |

The card should not assume that `hasThumbnail=true` guarantees the thumbnail is still available. Handle `404`, `410`, and network failures gracefully.

### 14.2 MARS

Use React Native `Image` for thumbnail rendering, but do not rely on `Linking.openURL` as the only download path. Use a native file download implementation and an explicit save/share destination. Preserve platform behavior:
- iOS: save/share through supported document/share APIs.
- Android: app cache or Downloads via supported filesystem/MediaStore APIs.
- Web: browser download behavior if the same app is built for web.

### 14.3 ADTP

Use a safe DOM component or carefully escaped HTML. Prefer DOM node creation and `textContent` for untrusted strings over interpolating raw HTML. Validate `serviceUrl` against known hosts before building `img.src` or anchor hrefs. Set `rel="noopener noreferrer"` for external targets. Use a Tauri command to save files rather than depending on a browser tab to download to an arbitrary local folder.

---

## 15. Error handling and retry policy

Use stable machine-readable error codes:

| HTTP | Code | Meaning | Client behavior |
|---|---|---|---|
| 400 | `INVALID_MEDIA_METADATA` | Invalid name/MIME/size/dimensions | Do not retry |
| 401 | `UNAUTHENTICATED` | Missing/invalid token | Refresh session or re-pair |
| 403 | `MEDIA_ACCESS_DENIED` | Caller is not sender/recipient | Do not retry |
| 404 | `MEDIA_NOT_FOUND` | Unknown ID/object | Refresh message metadata |
| 410 | `MEDIA_EXPIRED` | Retention deadline passed | Show expired card |
| 413 | `MEDIA_TOO_LARGE` | File exceeds policy | Ask user to select smaller file |
| 415 | `UNSUPPORTED_MEDIA_TYPE` | Type not allowed | Do not retry |
| 409 | `UPLOAD_STATE_CONFLICT` | Wrong lifecycle transition | Refresh transfer status |
| 422 | `MEDIA_INTEGRITY_FAILED` | Checksum/size/type mismatch | Delete partial object; restart |
| 429 | `RATE_LIMITED` | Too many requests | Exponential backoff |
| 502 | `BLOB_STORAGE_ERROR` | Upstream Blob failure | Retry with backoff |
| 503 | `MEDIA_SERVICE_UNAVAILABLE` | Relay/configuration unavailable | Keep local draft and retry |

Retry rules:
- Retry network failures and `5xx` with exponential backoff and jitter.
- Do not retry `4xx` except `401` after a successful token refresh.
- If a signed URL expires, request a fresh grant; do not reuse an expired URL indefinitely.
- Use the same idempotency key for retries of the same logical upload.
- Never send a second chat message merely because the upload completion response timed out; query upload state first.
- If upload succeeds but chat delivery fails, keep the media in a short-lived pending-send state and provide a safe retry path.

---

## 16. Observability and operations

Structured logs should include:
- `mediaId`
- `ownerId` (non-secret stable ID)
- `conversationId` (if allowed by privacy policy)
- `direction` (`mars_to_adtp` or `adtp_to_mars`)
- `mimeCategory`
- `originalSize`
- `storedSize`
- `durationMs`
- `status`
- `errorCode`
- `storageProvider`
- `createdAt` and `expiresAt`

Never log:
- access tokens;
- Blob tokens;
- full signed URLs;
- file contents;
- encryption keys;
- unredacted sensitive filenames if the product's privacy policy prohibits it.

Metrics:
- upload success/failure rate;
- upload duration by size bucket;
- compression ratio;
- thumbnail generation duration/failure rate;
- signed URL mint latency;
- download success/failure rate;
- Blob storage bytes and bandwidth;
- pending upload count;
- expired media count;
- cleanup lag;
- orphaned Blob object count;
- checksum mismatch count;
- retry count;
- memory usage during upload and image processing.

Alerts:
- cleanup lag > 30 minutes;
- repeated Blob 5xx/429 responses;
- cleanup worker unavailable;
- storage quota/budget threshold exceeded;
- upload verification failures above baseline;
- relay/ADTP authentication failures above baseline.

---

## 17. Testing strategy

### 17.1 Unit tests

MARS:
- image orientation and max-edge bounds;
- JPEG/PNG/GIF/HEIC behavior;
- thumbnail max dimensions;
- MIME and extension alignment;
- MediaRef v1/v2 normalization;
- unsafe URL rejection;
- expired card rendering;
- upload idempotency;
- local temp-file cleanup.

ADTP:
- `MediaRef` serde fixture compatibility;
- Pydantic validation;
- Pillow output MIME/filename consistency;
- streaming size limit enforcement;
- safe filename normalization;
- expiry calculations;
- checksum verification;
- authorization ownership checks;
- download response headers.

Relay:
- signed upload grant generation;
- wrong owner cannot complete;
- missing object cannot become ready;
- wrong content length rejected;
- wrong content type rejected;
- duplicate complete is idempotent;
- expired media cannot mint read URLs;
- only authorized recipient can mint download URL;
- cleanup removes original and thumbnail;
- cleanup retry works after partial failure;
- stale multipart upload cleanup;
- Redis unavailable produces safe failure, not false success.

### 17.2 Integration tests

1. MARS uploads a JPEG and ADTP receives a `MediaRef`.
2. ADTP downloads the original and verifies checksum.
3. ADTP uploads an image and MARS receives a `MediaRef`.
4. MARS downloads and verifies the original.
5. MARS uploads a PDF; no lossy compression occurs.
6. ADTP uploads a PNG with transparency; MIME/extension are preserved or explicitly converted to matching JPEG metadata.
7. A large file uses multipart/resumable upload.
8. An upload interrupted mid-transfer can resume or be safely restarted.
9. A user from another account cannot download a media ID.
10. A recipient cannot download media addressed to a different device.
11. Media is not downloadable after the 24-hour expiry.
12. Cleanup deletes both original and thumbnail.
13. An old chat message renders an expired state after cleanup.
14. Blob storage outage leaves a retryable draft and does not send a broken `MediaRef`.
15. A signed URL expiry during download causes a fresh URL request.

### 17.3 End-to-end acceptance tests

Run on physical Android and iOS devices plus supported desktop platforms (Windows, macOS, Linux).

- Phone → desktop: image, video, audio, PDF, generic file.
- Desktop → phone: image, video, audio, PDF, generic file.
- Both directions with desktop offline during send.
- Both directions with phone offline during send.
- Slow network, connection interruption, app backgrounding, app restart.
- Duplicate send tap.
- Cancel upload.
- Cancel download.
- Expired object.
- Corrupt/incorrect media metadata.
- Unauthorized user/device.
- Maximum allowed file size.
- Storage cleanup worker restart and retry.

---

## 18. Implementation plan and ordered work packages

Do the work in the order below. Do not start by changing both clients simultaneously; first stabilize the contract and service.

### Phase 0 — Baseline and safeguards

- [ ] Confirm actual deployed Upstash products: Blob bucket and Redis database are separate resources.
- [ ] Confirm `Mars` bucket is private.
- [ ] Confirm MARS relay owns `UPSTASH_BLOB_TOKEN`.
- [ ] Confirm ADTP owns only `ADTP_MEDIA_TOKEN` and `ADTP_RELAY_URL`.
- [ ] Capture current default branch commit SHAs and create feature branches.
- [ ] Add a media architecture integration test baseline.
- [ ] Record current upload limits, actual storage usage, and retention expectations.
- [ ] Verify whether current MARS and ADTP deployments use the same relay URL and bucket.

**Exit criteria:** secrets and infrastructure ownership are documented; existing behavior is reproducible.

### Phase 1 — Contract and storage registry

- [ ] Define `MediaRef v2` schema.
- [ ] Add compatibility parsing for current v1 `MediaRef`.
- [ ] Define Redis key schema and state machine.
- [ ] Implement Redis metadata repository.
- [ ] Add atomic state transitions and idempotency handling.
- [ ] Add expiry sorted set.
- [ ] Add contract fixtures shared across TypeScript, Python, and Rust.

**Exit criteria:** all three code layers serialize/deserialize the same contract; metadata survives relay restarts and multiple relay instances.

### Phase 2 — Harden shared media API

- [ ] Implement canonical `POST /api/v1/media/uploads`.
- [ ] Implement canonical `POST /api/v1/media/uploads/{id}/complete`.
- [ ] Implement authorized metadata lookup.
- [ ] Implement authorized download URL minting.
- [ ] Implement owner delete/cancel.
- [ ] Verify object existence, size, content type, and checksum before `ready`.
- [ ] Keep legacy routes as adapters.
- [ ] Add rate limits and per-owner quotas.
- [ ] Add stable error codes.
- [ ] Add structured audit logs without secrets or signed URLs.

**Exit criteria:** relay is the single source of truth for media lifecycle and authorization.

### Phase 3 — MARS mobile sender

- [ ] Make on-device compression the primary image path.
- [ ] Fix orientation-aware resizing and max-edge enforcement.
- [ ] Generate thumbnail before upload.
- [ ] Preserve animated/transparency-sensitive formats according to policy.
- [ ] Replace Base64 whole-file buffering for large media.
- [ ] Add upload progress, cancellation, retry, and idempotency.
- [ ] Upload directly to private Blob through signed grants.
- [ ] Send only `MediaRef` through `chat_message`.
- [ ] Keep gateway upload as a controlled compatibility fallback, not a divergent primary implementation.

**Exit criteria:** phone sends image and generic file to ADTP without media bytes crossing the chat transport.

### Phase 4 — ADTP desktop sender

- [ ] Fix Pillow output MIME/filename mismatch.
- [ ] Replace unbounded `UploadFile.read()` with bounded streaming/temp-file handling.
- [ ] Add checksum and object verification.
- [ ] Route every desktop/agent/device media sender through the canonical media service.
- [ ] Ensure `chat_response.media` and `device_message.media` use v2 contract.
- [ ] Remove duplicated upload semantics where possible.
- [ ] Ensure the paired phone receives a reachable service URL.

**Exit criteria:** ADTP can send media to MARS through the same Blob lifecycle and contract.

### Phase 5 — Recipient UI and native downloads

- [ ] Update MARS `MediaCard` for v2, progress, failure, and expiry states.
- [ ] Implement native MARS save/share download.
- [ ] Update ADTP `ChatPanel` and `DeviceChat` rendering.
- [ ] Replace unsafe HTML interpolation with safe DOM rendering or strict escaping.
- [ ] Implement Tauri/Rust download-to-path command.
- [ ] Verify checksum after download.
- [ ] Show expired media state after retention.

**Exit criteria:** both recipients preview and save files locally; neither depends on a permanent Blob URL.

### Phase 6 — Cleanup and lifecycle

- [ ] Implement cleanup worker using Redis expiry index.
- [ ] Delete original and thumbnail objects from Blob.
- [ ] Make deletion idempotent and retryable.
- [ ] Add stale upload and stale multipart cleanup.
- [ ] Add reconciliation job for orphaned objects.
- [ ] Add tombstone/expired behavior for old chat messages.
- [ ] Add cleanup metrics and alerts.

**Exit criteria:** a test object and its thumbnail are deleted within the configured cleanup SLO after 24 hours.

### Phase 7 — Security, load and release

- [ ] Complete sender/recipient authorization matrix.
- [ ] Verify no client contains Blob or ADTP service secrets.
- [ ] Verify signed URLs are never logged or persisted in chat.
- [ ] Test malicious MIME, filename, URL, and oversized file cases.
- [ ] Test concurrent uploads and downloads.
- [ ] Test Redis/Blob outages and cleanup worker restart.
- [ ] Run Android, iOS, Windows, macOS, and Linux smoke tests.
- [ ] Run repository lint, typecheck, unit, integration, and build pipelines.
- [ ] Document deployment environment variables, rollback, and retention settings.

**Exit criteria:** all acceptance tests pass and operations can observe/recover the pipeline.

---

## 19. Required environment variables

### MARS relay server only

```dotenv
UPSTASH_BLOB_TOKEN=<private-blob-bucket-token>
UPSTASH_REDIS_REST_URL=<upstash-redis-rest-url>
UPSTASH_REDIS_REST_TOKEN=<upstash-redis-rest-token>

ADTP_MEDIA_TOKEN=<long-random-server-to-server-secret>
RELAY_PUBLIC_URL=https://media.example.com

MEDIA_RETENTION_SECONDS=86400
MEDIA_UPLOAD_GRANT_SECONDS=600
MEDIA_READ_URL_SECONDS=300
MEDIA_MAX_IMAGE_BYTES=52428800
MEDIA_MAX_VIDEO_BYTES=209715200
MEDIA_MAX_DOCUMENT_BYTES=209715200
MEDIA_CLEANUP_INTERVAL_SECONDS=600
```

### ADTP FastAPI server only

```dotenv
ADTP_RELAY_URL=https://media.example.com
ADTP_MEDIA_TOKEN=<same-server-to-server-secret>
ADTP_MAX_MEDIA_SIZE=209715200
ADTP_MEDIA_UPLOAD_TIMEOUT_SECONDS=300
```

### MARS mobile app

```dotenv
EXPO_PUBLIC_BACKEND_URL=https://media.example.com
```

The mobile app must not receive:
- `UPSTASH_BLOB_TOKEN`
- `UPSTASH_REDIS_REST_TOKEN`
- `ADTP_MEDIA_TOKEN`
- Supabase service-role key

Do not commit populated `.env` files. Use deployment secret managers and rotate credentials.

---

## 20. Definition of done

The feature is complete only when all items below are true.

### User experience
- [ ] A user can select media in MARS and send it to ADTP.
- [ ] A user can select media in ADTP and send it to MARS.
- [ ] Supported images are compressed before upload.
- [ ] Image thumbnails are generated before upload.
- [ ] Non-image files are preserved without lossy transformation.
- [ ] Both clients render a media card with a preview or file-type tile.
- [ ] The download action saves the original file locally.
- [ ] Download progress, cancellation, retry, and failure are visible.
- [ ] Expired media is clearly marked and cannot be downloaded.

### Architecture
- [ ] Both directions use the same `MediaRef` v2 contract.
- [ ] Blob stores bytes; Redis stores metadata and expiry coordination.
- [ ] Chat transports carry metadata only.
- [ ] The MARS relay is the authoritative media service.
- [ ] ADTP authenticates to the relay server-to-server.
- [ ] No client receives storage credentials.
- [ ] Large files use bounded streaming or multipart/resumable upload.
- [ ] Upload completion verifies actual stored objects.

### Lifecycle
- [ ] Every media object has a server-generated `expiresAt`.
- [ ] Original and thumbnail share the same expiry.
- [ ] Cleanup runs automatically and retries failures.
- [ ] Objects are deleted within the defined cleanup SLO after the 24-hour deadline.
- [ ] Redis metadata and indexes are cleaned consistently.
- [ ] Stale multipart uploads are aborted.
- [ ] Chat history handles expired references correctly.

### Security and quality
- [ ] Sender and recipient authorization is enforced.
- [ ] Signed URLs are short-lived and not persisted.
- [ ] MIME, size, filename, URL, and image dimensions are validated.
- [ ] Downloaded files are checksum-verified.
- [ ] No untrusted file is automatically executed or rendered as active content.
- [ ] No secrets or signed URLs appear in logs.
- [ ] Unit, integration, and cross-platform end-to-end tests pass.

---

## 21. Priority fixes: highest-risk items first

| Priority | Finding | Required fix |
|---|---|---|
| P0 | MARS has a direct Blob path and a separate gateway multipart path with different compression behavior | Make the compressed direct-to-Blob path canonical; retain gateway only as a controlled adapter/fallback |
| P0 | No actual 24-hour Blob object deletion is implemented in the reviewed relay flow | Add Redis expiry index + scheduled cleanup worker + Blob deletion |
| P0 | MARS media index is process-local Map + JSON file | Replace with Redis-backed metadata and atomic state transitions |
| P0 | Header-free file route behaves as a bearer capability | Introduce recipient authorization before minting download URLs, or formally scope capability URLs |
| P0 | ADTP marks upload complete without an explicit object verification step | Verify object existence, size, type, and checksum before `ready` |
| P1 | ADTP converts images to JPEG but can retain original MIME/name | Return actual output MIME/extension from image preparation |
| P1 | MARS Base64 conversion and ADTP full-file buffering can exhaust memory | Implement streaming/native file upload and bounded temporary-file processing |
| P1 | MARS resize uses fixed width and may violate max-edge for portrait photos | Calculate orientation-aware target dimensions |
| P1 | Desktop download behavior is primarily an anchor/open URL | Add native Tauri/Rust save-to-disk command with checksum verification |
| P1 | MediaRef has no schema version or explicit expiry | Introduce v2 with compatibility migration |
| P1 | Large uploads lack a complete resumable/multipart lifecycle | Add multipart upload, resume, cancel, and stale multipart cleanup |
| P2 | Video/audio thumbnails and metadata are limited | Add poster frame extraction and media metadata after image/document pipeline is stable |
| P2 | Shared wire type is duplicated across TypeScript/Python/Rust | Add canonical schema and cross-language fixture tests |
| P2 | Frontend HTML string rendering increases injection risk | Use safe DOM rendering and strict URL validation |

---

## 22. Final architecture decision

**Adopt one media control plane and one shared data contract.**

- Upstash Blob is the private object store.
- Upstash Redis is the metadata/expiry index.
- MARS relay owns the storage credentials and media lifecycle.
- MARS performs supported image compression and thumbnail generation on the phone.
- ADTP performs supported image compression and thumbnail generation on the desktop.
- Both clients upload to the same private Blob bucket using short-lived signed grants.
- Both clients send only `MediaRef` metadata through chat.
- Both recipients request authorized short-lived download URLs and save the original locally.
- A scheduled cleanup worker deletes the original and thumbnail after 24 hours.
- Expired chat references remain safe and render as expired rather than producing broken download links.

This architecture preserves the existing React Native, Rust/Tauri, and FastAPI boundaries while giving MARS and ADTP one consistent, testable, secure media-transfer protocol.

---

## 23. Reference documentation

### Repository implementation
- MARS media pipeline: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/docs/MEDIA_PIPELINE.md
- MARS media relay: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/relay/media.js
- MARS compression: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/compress.ts
- MARS Blob client: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/blobClient.ts
- MARS gateway upload: https://github.com/ASHIR-TECH/M.A.R.S-Sourcecode/blob/main/mobile/src/media/gatewayUpload.ts
- ADTP media router: https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/routers/media.py
- ADTP media service: https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/services/media_service.py
- ADTP media model: https://github.com/ASHIR-TECH/A.D.T.P/blob/main/packages/adtp-api/src/adtp_api/models/media.py
- ADTP mobile handoff: https://github.com/ASHIR-TECH/A.D.T.P/blob/main/MOBILE_HANDOFF.md

### Upstash
- Blob product overview: https://upstash.com/blob
- Blob quickstart: https://upstash.com/docs/blob/overall/quickstart
- Blob signed URL behavior: https://upstash.com/docs/blob/reference/signing
- Blob private reads: https://upstash.com/docs/blob/bucket/reading
- Blob private documents pattern: https://upstash.com/docs/blob/recipes/private-documents
- Redis REST API: https://upstash.com/docs/redis/features/restapi
- Redis pricing and limits: https://upstash.com/pricing/redis

### Platform engineering
- Expo ImageManipulator: https://docs.expo.dev/versions/latest/sdk/imagemanipulator/
- React Native Image: https://reactnative.dev/docs/image
- Tokio I/O: https://tokio.rs/tokio/tutorial/io
- Tokio tutorial: https://tokio.rs/tokio/tutorial
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Tauri filesystem plugin: https://v2.tauri.app/plugin/file-system/

### WhatsApp / Meta reference
- WhatsApp privacy policy (temporary encrypted media storage): https://www.whatsapp.com/legal/privacy-policy-eea?lang=en_gb
- WhatsApp security advisories: https://www.whatsapp.com/security/advisories?lang=en_US
- Meta engineering: Rust media consistency/security work in WhatsApp: https://engineering.fb.com/2026/01/27/security/rust-at-scale-security/
- Messenger E2EE media attachment overview (related Meta architecture reference): https://engineering.fb.com/wp-content/uploads/2023/12/MessengerEnd-to-EndEncryptionOverview_12-6-2023.pdf

---

## Review limitations

This document is a source-based architecture review of the publicly accessible repository files listed above. It does not claim that the feature has been implemented, deployed, load-tested, or penetration-tested. It does not inspect private production configuration, live Upstash bucket settings, database contents, or uncommitted local changes. Before implementation, confirm the exact deployed branch and run the acceptance tests against the active relay and bucket.
