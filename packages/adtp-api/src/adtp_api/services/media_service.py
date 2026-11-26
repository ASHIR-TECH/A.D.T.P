"""Compress media and store it in the shared Upstash Blob bucket.

ADTP never sees the bucket token. It authenticates to the MARS relay's
``/api/media/*`` API with ADTP_MEDIA_TOKEN, uploads bytes directly to the
signed grant, and only passes the returned MediaRef through chat/device
messages.

ADTP-side v2 safeguards implemented here:

* image preparation returns the *actual* output MIME type and filename, so a
  PNG converted to JPEG is never advertised as ``image/png``;
* uploads are spooled to a bounded temporary file and hashed while streaming,
  instead of ``await file.read()`` holding the whole file in memory;
* the final object is verified through the relay metadata endpoint before the
  MediaRef is treated as usable;
* no secret, signed URL, or file content is written to logs.
"""
from __future__ import annotations

import asyncio
import hashlib
import io
import logging
import os
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, AsyncIterator, BinaryIO

import httpx
from fastapi import HTTPException, UploadFile

from adtp_api.config import settings
from adtp_api.models.media import MediaRef, normalize_media_ref

log = logging.getLogger(__name__)

MAX_IMAGE_EDGE = 1600
THUMBNAIL_EDGE = 320
IMAGE_QUALITY = 72
THUMBNAIL_QUALITY = 60
# Refuse to decode absurd images (decompression-bomb guard).
MAX_IMAGE_PIXELS = 40_000_000
STREAM_CHUNK = 64 * 1024

_MIME_EXTENSIONS = {
    "image/jpeg": ".jpg",
    "image/png": ".png",
}
_UNSAFE_NAME_CHARS = re.compile(r"[\x00-\x1f\x7f/\\]")


def _relay_base() -> str:
    return (settings.relay_url or "").rstrip("/")


def configured() -> bool:
    return bool(_relay_base() and settings.media_token)


def _auth_headers() -> dict[str, str]:
    return {"Authorization": f"Bearer {settings.media_token}"}


def safe_display_name(name: str | None) -> str:
    """Normalise a client filename to a safe display/stored basename."""
    text = (name or "").strip().replace("\\", "/")
    base = text.rsplit("/", 1)[-1]
    cleaned = _UNSAFE_NAME_CHARS.sub("_", base).strip(" .")[:200]
    return cleaned or "file.bin"


def _with_extension(name: str, mime_type: str) -> str:
    stem = os.path.splitext(safe_display_name(name))[0] or "media"
    return stem + _MIME_EXTENSIONS.get(mime_type, ".bin")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class PreparedMedia:
    """The bytes actually stored and their truthful metadata."""

    data: bytes | Path
    thumbnail: bytes | None
    width: int | None
    height: int | None
    compressed: bool
    mime_type: str
    name: str


class _PassThroughImage(Exception):
    """Animated/transparent/huge image: keep original bytes rather than damage it."""


def compress_image(
    source: bytes | Path,
    mime_type: str,
) -> tuple[bytes, bytes, int, int, str]:
    """Return (compressed, thumbnail, width, height, output_mime).

    Photographic JPEG input becomes JPEG. Images with alpha are re-encoded as
    PNG so transparency survives. Animated images and decompression bombs are
    passed through untouched by raising :class:`_PassThroughImage`.
    """
    from PIL import Image

    if isinstance(source, (bytes, bytearray, memoryview)):
        source = io.BytesIO(source)
    with Image.open(source) as image:
        image.load()
        width, height = image.size
        if width * height > MAX_IMAGE_PIXELS:
            raise _PassThroughImage("image exceeds pixel budget")
        if getattr(image, "is_animated", False) or getattr(image, "n_frames", 1) > 1:
            raise _PassThroughImage("animated image")

        has_alpha = image.mode in ("RGBA", "LA") or "transparency" in image.info
        output_mime = "image/png" if has_alpha else "image/jpeg"
        working = image.copy()
        if not has_alpha and working.mode not in ("RGB", "L"):
            working = working.convert("RGB")
        longest = max(width, height)
        if longest > MAX_IMAGE_EDGE:
            scale = MAX_IMAGE_EDGE / longest
            size = (max(1, round(width * scale)), max(1, round(height * scale)))
            working = working.resize(size, Image.Resampling.LANCZOS)
            width, height = size

        main = io.BytesIO()
        if output_mime == "image/jpeg":
            working.save(main, format="JPEG", quality=IMAGE_QUALITY, optimize=True)
        else:
            working.save(main, format="PNG", optimize=True)

        thumb = io.BytesIO()
        small = working.copy()
        small.thumbnail((THUMBNAIL_EDGE, THUMBNAIL_EDGE), Image.Resampling.LANCZOS)
        if output_mime == "image/jpeg":
            small.save(thumb, format="JPEG", quality=THUMBNAIL_QUALITY)
        else:
            small.save(thumb, format="PNG", optimize=True)

        return main.getvalue(), thumb.getvalue(), width, height, output_mime


def prepare(source: bytes | Path, mime_type: str, name: str) -> PreparedMedia:
    """Compress supported images; pass other media through unchanged."""
    if mime_type.startswith("image/") and mime_type != "image/svg+xml":
        try:
            main, thumb, width, height, output_mime = compress_image(source, mime_type)
            return PreparedMedia(
                data=main,
                thumbnail=thumb,
                width=width,
                height=height,
                compressed=True,
                mime_type=output_mime,
                name=_with_extension(name, output_mime),
            )
        except (_PassThroughImage, OSError, ValueError) as exc:
            log.warning("image preparation skipped (%s); storing original", type(exc).__name__)
        except Exception as exc:  # noqa: BLE001 - never fail an upload on PIL quirks
            log.warning("image preparation failed (%s); storing original", type(exc).__name__)
    return PreparedMedia(
        data=source,
        thumbnail=None,
        width=None,
        height=None,
        compressed=False,
        mime_type=mime_type or "application/octet-stream",
        name=safe_display_name(name),
    )


async def _iter_file(path: Path) -> AsyncIterator[bytes]:
    """Stream a spooled file without blocking the event loop for long reads."""
    loop = asyncio.get_running_loop()
    with open(path, "rb") as handle:
        while True:
            chunk = await loop.run_in_executor(None, handle.read, STREAM_CHUNK)
            if not chunk:
                break
            yield chunk


async def _spool_upload(file: UploadFile) -> tuple[Path, int, str]:
    """Copy an UploadFile to a temp file with a hard size cap and streamed hash."""
    fd, raw_path = tempfile.mkstemp(prefix="adtp-media-", suffix=".upload")
    path = Path(raw_path)
    size = 0
    sha = hashlib.sha256()
    try:
        with os.fdopen(fd, "wb") as handle:
            while True:
                chunk = await file.read(STREAM_CHUNK)
                if not chunk:
                    break
                size += len(chunk)
                if size > settings.max_media_size:
                    raise HTTPException(413, "File is too large")
                sha.update(chunk)
                handle.write(chunk)
        if size == 0:
            raise HTTPException(400, "Empty file")
        return path, size, sha.hexdigest()
    except BaseException:
        await asyncio.to_thread(path.unlink, missing_ok=True)
        raise


async def _remove(path: Path) -> None:
    await asyncio.to_thread(path.unlink, missing_ok=True)


async def store_path(
    path: Path,
    *,
    name: str,
    mime_type: str,
    size: int,
    sha256: str | None = None,
    source_sha256: str | None = None,
    width: int | None = None,
    height: int | None = None,
    sender: str | None = None,
    session_id: str | None = None,
    conversation_id: str | None = None,
    recipient: str | None = None,
    generate_thumbnail: bool = True,
) -> MediaRef:
    """Compress -> signed PUT -> complete -> verify. Returns only a MediaRef."""
    base = _relay_base()
    if not configured():
        raise HTTPException(503, "Media storage is not configured (ADTP_RELAY_URL / ADTP_MEDIA_TOKEN)")

    prepared = await asyncio.to_thread(prepare, path, mime_type, name)
    thumbnail = prepared.thumbnail if generate_thumbnail else None
    stored_size = len(prepared.data) if isinstance(prepared.data, bytes) else size
    stored_sha = digest(prepared.data) if isinstance(prepared.data, bytes) else (sha256 or source_sha256)
    width = width or prepared.width
    height = height or prepared.height

    async with httpx.AsyncClient(timeout=60.0) as client:
        try:
            sign = await client.post(
                f"{base}/api/media/sign",
                headers={"Content-Type": "application/json", **_auth_headers()},
                json={
                    "name": prepared.name,
                    "mimeType": prepared.mime_type,
                    "size": size,
                    "storedSize": stored_size,
                    "compressedSize": stored_size,
                    "width": width,
                    "height": height,
                    "compressed": prepared.compressed,
                    "thumbnail": bool(thumbnail),
                    "sha256": stored_sha,
                    "sender": sender or "adtp-ai",
                    "recipient": recipient,
                    "conversationId": conversation_id,
                    "sessionId": session_id,
                },
            )
            sign.raise_for_status()
            grant = sign.json()
        except httpx.HTTPError as exc:
            log.error("media sign failed (status=%s)", getattr(exc.response, "status_code", None))
            raise HTTPException(502, "Could not prepare media upload") from exc

        media_id = grant.get("id")
        original_grant = grant.get("original")
        if not media_id or not isinstance(original_grant, dict):
            raise HTTPException(502, "Media service returned an invalid grant")

        uploads: list[tuple[bytes | Path, dict[str, Any]]] = [
            (prepared.data, original_grant),
        ]
        if thumbnail is not None and isinstance(grant.get("thumbnail"), dict):
            uploads.append((thumbnail, grant["thumbnail"]))

        for body, upload in uploads:
            url = upload.get("url")
            if not isinstance(url, str) or not url.startswith(("http://", "https://")):
                raise HTTPException(502, "Media service returned an invalid upload grant")
            headers = upload.get("headers") or {}
            try:
                if isinstance(body, bytes):
                    put = await client.put(url, headers=headers, content=body)
                else:
                    put = await client.put(url, headers=headers, content=_iter_file(body))
                put.raise_for_status()
            except httpx.HTTPError as exc:
                # Never log the URL: a signed URL is a bearer credential.
                log.error("blob PUT failed (status=%s)", getattr(exc.response, "status_code", None))
                raise HTTPException(502, "Could not upload media bytes") from exc

        try:
            done = await client.post(
                f"{base}/api/media/complete",
                headers={"Content-Type": "application/json", **_auth_headers()},
                json={
                    "id": media_id,
                    "storedSize": stored_size,
                    "compressedSize": stored_size,
                    "size": size,
                    "width": width,
                    "height": height,
                    "mimeType": prepared.mime_type,
                    "name": prepared.name,
                    "thumbnailUploaded": thumbnail is not None,
                    "sha256": stored_sha,
                },
            )
            done.raise_for_status()
            media = normalize_media_ref(done.json().get("media"))
        except (httpx.HTTPError, ValueError) as exc:
            log.error("media complete failed (status=%s)", getattr(exc.response, "status_code", None))
            raise HTTPException(502, "Could not finalize media upload") from exc

        if not media:
            raise HTTPException(502, "Media service returned an invalid reference")

        # The relay is the authority for lifecycle, but ADTP must not advertise
        # bytes it did not actually store. Verify metadata before the ref is
        # allowed into chat.
        try:
            check = await client.get(
                f"{base}/api/media/{media_id}",
                headers=_auth_headers(),
            )
            check.raise_for_status()
            stored = normalize_media_ref(check.json().get("media"))
        except httpx.HTTPError as exc:
            log.error("media verification failed (status=%s)", getattr(exc.response, "status_code", None))
            raise HTTPException(422, "MEDIA_INTEGRITY_FAILED: could not verify stored media") from exc
        if stored is not None:
            stored_size_actual = stored.storedSize or stored.compressedSize
            if stored_size_actual and stored_size_actual != stored_size:
                raise HTTPException(422, "MEDIA_INTEGRITY_FAILED: stored size mismatch")
            if stored.sha256 and stored_sha and stored.sha256 != stored_sha:
                raise HTTPException(422, "MEDIA_INTEGRITY_FAILED: checksum mismatch")

    return normalize_media_ref({
        **media.as_wire(),
        "schemaVersion": 2,
        "storedSize": stored_size,
        "compressedSize": stored_size,
        "name": prepared.name,
        "mimeType": prepared.mime_type,
        "sha256": stored_sha,
        "size": size,
        "width": width,
        "height": height,
        "compressed": prepared.compressed,
        "hasThumbnail": thumbnail is not None,
        "status": "ready",
        "conversationId": conversation_id,
        "recipient": recipient,
    }) or media


async def store(
    data: bytes,
    *,
    name: str,
    mime_type: str,
    original_size: int | None = None,
    width: int | None = None,
    height: int | None = None,
    sender: str | None = None,
    session_id: str | None = None,
    conversation_id: str | None = None,
    recipient: str | None = None,
    generate_thumbnail: bool = True,
) -> MediaRef:
    """Compatibility wrapper for callers that already hold bounded bytes."""
    if not data:
        raise HTTPException(400, "Empty file")
    if len(data) > settings.max_media_size:
        raise HTTPException(413, "File is too large")
    fd, raw_path = tempfile.mkstemp(prefix="adtp-media-", suffix=".upload")
    path = Path(raw_path)
    try:
        await asyncio.to_thread(_write_all, fd, data)
        return await store_path(
            path,
            name=name,
            mime_type=mime_type,
            size=original_size if original_size is not None else len(data),
            sha256=digest(data),
            width=width,
            height=height,
            sender=sender,
            session_id=session_id,
            conversation_id=conversation_id,
            recipient=recipient,
            generate_thumbnail=generate_thumbnail,
        )
    finally:
        await _remove(path)


def _write_all(fd: int, data: bytes) -> None:
    with os.fdopen(fd, "wb") as handle:
        handle.write(data)


async def store_upload(file: UploadFile, **meta: Any) -> MediaRef:
    """Stream an uploaded file to disk, then run the bounded store pipeline."""
    path, size, sha = await _spool_upload(file)
    try:
        return await store_path(
            path,
            name=meta.pop("name", None) or file.filename or "file.bin",
            mime_type=meta.pop("mime_type", None) or file.content_type or "application/octet-stream",
            size=size,
            sha256=sha,
            **meta,
        )
    finally:
        await _remove(path)


async def fetch(media_id: str) -> MediaRef:
    """Rehydrate a MediaRef from the relay (for old persisted messages)."""
    base = _relay_base()
    if not base:
        raise HTTPException(503, "Media storage is not configured (ADTP_RELAY_URL)")
    try:
        async with httpx.AsyncClient(timeout=15.0) as client:
            response = await client.get(f"{base}/api/media/{media_id}", headers=_auth_headers())
            response.raise_for_status()
            media = normalize_media_ref(response.json().get("media"))
    except httpx.HTTPError as exc:
        log.error("media metadata lookup failed (status=%s)", getattr(exc.response, "status_code", None))
        raise HTTPException(502, "Could not read media metadata") from exc
    if not media:
        raise HTTPException(404, "Media not found")
    return media
