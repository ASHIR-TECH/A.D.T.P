"""Compress media and store it in the shared Upstash Blob bucket.

ADTP never sees the bucket token. It authenticates to the MARS relay's
/api/media/* API with ADTP_MEDIA_TOKEN, uploads bytes directly to the signed
grant, and only passes the returned MediaRef through chat/device messages.
"""
from __future__ import annotations

import hashlib
import io
import logging
from typing import Any

import httpx
from fastapi import HTTPException, UploadFile

from adtp_api.config import settings
from adtp_api.models.media import MediaRef, normalize_media_ref

log = logging.getLogger(__name__)

MAX_IMAGE_EDGE = 1600
THUMBNAIL_EDGE = 320
IMAGE_QUALITY = 72
THUMBNAIL_QUALITY = 60


def _relay_base() -> str:
    return (settings.relay_url or "").rstrip("/")


def configured() -> bool:
    return bool(_relay_base() and settings.media_token)


def _auth_headers() -> dict[str, str]:
    return {"Authorization": f"Bearer {settings.media_token}"}


def compress_image(data: bytes, mime_type: str) -> tuple[bytes, bytes | None, int, int]:
    """Return (compressed, thumbnail, width, height). Falls back on bad images."""
    from PIL import Image

    with Image.open(io.BytesIO(data)) as image:
        image.load()
        width, height = image.size
        longest = max(width, height)
        working = image.convert("RGB") if image.mode not in ("RGB", "L") else image.copy()
        if longest > MAX_IMAGE_EDGE:
            scale = MAX_IMAGE_EDGE / longest
            size = (max(1, round(width * scale)), max(1, round(height * scale)))
            working = working.resize(size, Image.Resampling.LANCZOS)
            width, height = size

        main = io.BytesIO()
        working.save(main, format="JPEG", quality=IMAGE_QUALITY, optimize=True)

        thumb = io.BytesIO()
        small = working.copy()
        small.thumbnail((THUMBNAIL_EDGE, THUMBNAIL_EDGE), Image.Resampling.LANCZOS)
        small.save(thumb, format="JPEG", quality=THUMBNAIL_QUALITY)

        return main.getvalue(), thumb.getvalue(), width, height


def prepare(data: bytes, mime_type: str, name: str) -> tuple[bytes, bytes | None, int | None, int | None, bool]:
    """Compress images; pass other media through unchanged (already compressed)."""
    if mime_type.startswith("image/"):
        try:
            compressed, thumb, width, height = compress_image(data, mime_type)
            return compressed, thumb, width, height, True
        except Exception:  # noqa: BLE001 - unsupported/animated formats pass through
            log.warning("image compression failed for %s; storing original", name)
    return data, None, None, None, False


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
    generate_thumbnail: bool = True,
) -> MediaRef:
    """Compress -> signed PUT -> complete. Returns only a MediaRef."""
    base = _relay_base()
    if not configured():
        raise HTTPException(503, "Media storage is not configured (ADTP_RELAY_URL / ADTP_MEDIA_TOKEN)")
    if not data:
        raise HTTPException(400, "Empty file")
    if len(data) > settings.max_media_size:
        raise HTTPException(413, "File is too large")

    compressed, thumb, comp_width, comp_height, did_compress = prepare(data, mime_type, name)
    if not generate_thumbnail:
        thumb = None
    width = width or comp_width
    height = height or comp_height

    async with httpx.AsyncClient(timeout=60.0) as client:
        try:
            response = await client.post(
                f"{base}/api/media/sign",
                headers={"Content-Type": "application/json", **_auth_headers()},
                json={
                    "name": name,
                    "mimeType": mime_type,
                    "size": original_size if original_size is not None else len(data),
                    "compressedSize": len(compressed),
                    "width": width,
                    "height": height,
                    "compressed": did_compress,
                    "thumbnail": bool(thumb),
                    "sender": sender or "adtp-ai",
                    "sessionId": session_id,
                },
            )
            response.raise_for_status()
            grant = response.json()
        except httpx.HTTPError as exc:
            log.error("media sign failed: %s", exc)
            raise HTTPException(502, "Could not prepare media upload") from exc

        media_id = grant.get("id")
        original_grant = grant.get("original")
        if not media_id or not isinstance(original_grant, dict):
            raise HTTPException(502, "Media service returned an invalid grant")

        for body, upload in ((compressed, original_grant), (thumb, grant.get("thumbnail"))):
            if body is None or not isinstance(upload, dict):
                continue
            try:
                put = await client.put(
                    upload["url"],
                    headers=upload.get("headers", {}),
                    content=body,
                )
                put.raise_for_status()
            except (httpx.HTTPError, KeyError) as exc:
                log.error("blob PUT failed: %s", exc)
                raise HTTPException(502, "Could not upload media bytes") from exc

        try:
            done = await client.post(
                f"{base}/api/media/complete",
                headers={"Content-Type": "application/json", **_auth_headers()},
                json={
                    "id": media_id,
                    "compressedSize": len(compressed),
                    "width": width,
                    "height": height,
                    "thumbnailUploaded": bool(thumb),
                },
            )
            done.raise_for_status()
            media = normalize_media_ref(done.json().get("media"))
        except (httpx.HTTPError, ValueError) as exc:
            log.error("media complete failed: %s", exc)
            raise HTTPException(502, "Could not finalize media upload") from exc

    if not media:
        raise HTTPException(502, "Media service returned an invalid reference")
    return media


async def store_upload(file: UploadFile, **meta: Any) -> MediaRef:
    data = await file.read()
    return await store(
        data,
        name=file.filename or "file.bin",
        mime_type=file.content_type or "application/octet-stream",
        original_size=len(data),
        **meta,
    )


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
        raise HTTPException(502, "Could not read media metadata") from exc
    if not media:
        raise HTTPException(404, "Media not found")
    return media


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()
