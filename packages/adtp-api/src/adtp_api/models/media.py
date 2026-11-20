"""Media reference shared by ADTP and the MARS mobile companion.

This is the wire contract: only this JSON metadata travels in chat/device
messages. Bytes are compressed, uploaded to the private Upstash Blob bucket
through the MARS relay, and read back through short-lived signed URLs.
"""
from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class MediaRef(BaseModel):
    """A stored media object. Mirrors mobile/src/media/types.ts exactly."""

    id: str
    serviceUrl: str
    name: str
    mimeType: str = "application/octet-stream"
    size: int = 0
    compressedSize: int = 0
    width: int | None = None
    height: int | None = None
    durationMs: int | None = None
    sha256: str | None = None
    sender: str | None = None
    sessionId: str | None = None
    compressed: bool = False
    hasThumbnail: bool = False
    createdAt: str = ""


class MediaUploadMeta(BaseModel):
    """Optional metadata accepted by POST /api/v1/media/upload."""

    sender: str | None = None
    sessionId: str | None = None
    generateThumbnail: bool = True


def normalize_media_ref(value: Any) -> MediaRef | None:
    """Harden arbitrary JSON from the agent socket / relay into a MediaRef."""
    if not isinstance(value, dict):
        return None
    media_id = value.get("id")
    name = value.get("name")
    if not isinstance(media_id, str) or not media_id or not isinstance(name, str):
        return None
    service_url = value.get("serviceUrl")
    if not isinstance(service_url, str) or not service_url.startswith(("http://", "https://")):
        return None
    try:
        return MediaRef.model_validate(value)
    except Exception:  # noqa: BLE001 - deliberately lossy at the trust boundary
        return None


def media_from_tool_calls(tool_calls: Any) -> MediaRef | None:
    """Find a MediaRef in a completed tool result (MARS does the same)."""
    import json

    if not isinstance(tool_calls, list):
        return None
    for call in tool_calls:
        if not isinstance(call, dict) or call.get("status") not in (None, "completed"):
            continue
        result = call.get("result")
        parsed: Any = result
        if isinstance(result, str):
            stripped = result.strip()
            if not stripped.startswith("{"):
                continue
            try:
                parsed = json.loads(stripped)
            except (ValueError, TypeError):
                continue
        if not isinstance(parsed, dict):
            continue
        found = normalize_media_ref(parsed.get("media") or parsed)
        if found:
            return found
    return None
