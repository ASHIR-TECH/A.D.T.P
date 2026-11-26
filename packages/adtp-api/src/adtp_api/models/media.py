"""Media reference shared by ADTP and the MARS mobile companion.

This is the wire contract: only this JSON metadata travels in chat/device
messages. Bytes are compressed, uploaded to the private Upstash Blob bucket
through the MARS relay, and read back through short-lived signed URLs.

Schema v2 adds explicit ``schemaVersion``, ``storedSize``, ``conversationId``,
``expiresAt`` and ``status`` fields. v1 payloads (``compressedSize``, no
version) are still accepted and upgraded on ingest so old chat messages keep
rendering.
"""
from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Literal
import re
from urllib.parse import urlsplit

from pydantic import BaseModel, Field, field_validator, model_validator

MediaStatus = Literal[
    "created", "uploading", "verifying", "ready", "failed",
    "cancelled", "deleting", "expired", "deleted",
]

TERMINAL_STATUSES = frozenset({"expired", "deleted"})


def _parse_instant(value: str) -> datetime | None:
    """Parse an ISO-8601/UTC expiry string; return None when unparseable."""
    if not value:
        return None
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


class MediaRef(BaseModel):
    """A stored media object. Mirrors the shared ADTP/MARS v2 contract.

    ``compressedSize`` remains as a deprecated read/write alias of
    ``storedSize`` for one compatibility release.
    """

    schemaVersion: int = Field(default=2, ge=1, le=2)
    id: str
    serviceUrl: str
    name: str
    mimeType: str = "application/octet-stream"
    size: int = Field(default=0, ge=0)
    storedSize: int = Field(default=0, ge=0)
    compressedSize: int = Field(default=0, ge=0)
    width: int | None = Field(default=None, ge=0)
    height: int | None = Field(default=None, ge=0)
    durationMs: int | None = Field(default=None, ge=0)
    sha256: str | None = None
    sender: str | None = None
    recipient: str | None = None
    conversationId: str | None = None
    sessionId: str | None = None
    compressed: bool = False
    hasThumbnail: bool = False
    createdAt: str = ""
    expiresAt: str = ""
    status: MediaStatus = "ready"

    @model_validator(mode="before")
    @classmethod
    def _upgrade_v1(cls, value: Any) -> Any:
        if not isinstance(value, dict):
            return value
        data = dict(value)
        # v1 only had compressedSize. Keep both fields in sync so old readers
        # and new readers see the same byte count.
        if "storedSize" not in data and "compressedSize" in data:
            data["storedSize"] = data["compressedSize"]
        if "compressedSize" not in data and "storedSize" in data:
            data["compressedSize"] = data["storedSize"]
        if "schemaVersion" not in data:
            data["schemaVersion"] = 1
        # The deployed relay uses `pending` / `complete`; map them onto the
        # canonical state machine so v1 messages keep normalising.
        status = data.get("status")
        if status in ("pending",):
            data["status"] = "created"
        elif status in ("complete", "completed"):
            data["status"] = "ready"
        return data

    @field_validator("id", "name")
    @classmethod
    def _non_empty(cls, value: str) -> str:
        if not value or len(value) > 255:
            raise ValueError("must be a non-empty string of at most 255 chars")
        if any(ch in value for ch in ("/", "\\")) or any(ord(ch) < 32 for ch in value):
            raise ValueError("must not contain path separators or control characters")
        return value

    @field_validator("mimeType")
    @classmethod
    def _mime_type(cls, value: str) -> str:
        if "/" not in value or len(value) > 150:
            raise ValueError("invalid media MIME type")
        return value

    @field_validator("sha256")
    @classmethod
    def _checksum(cls, value: str | None) -> str | None:
        if value is not None and not re.fullmatch(r"[a-fA-F0-9]{32,128}", value):
            raise ValueError("sha256 must be 32-128 hex characters")
        return value

    @field_validator("serviceUrl")
    @classmethod
    def _safe_service_url(cls, value: str) -> str:
        parsed = urlsplit(value)
        if parsed.scheme not in ("http", "https") or not parsed.hostname:
            raise ValueError("serviceUrl must be an http(s) URL")
        if parsed.username or parsed.password:
            raise ValueError("serviceUrl must not embed credentials")
        return value.rstrip("/")

    @field_validator("status")
    @classmethod
    def _known_status(cls, value: str) -> str:
        if value not in (
            "created", "uploading", "verifying", "ready", "failed",
            "cancelled", "deleting", "expired", "deleted",
        ):
            raise ValueError("unknown media status")
        return value

    @property
    def expires_at_dt(self) -> datetime | None:
        return _parse_instant(self.expiresAt)

    @property
    def expired(self) -> bool:
        """True when the object retention deadline has passed."""
        if self.status in TERMINAL_STATUSES:
            return True
        deadline = self.expires_at_dt
        return deadline is not None and deadline <= datetime.now(timezone.utc)

    def as_wire(self) -> dict[str, Any]:
        """Serialise with both size aliases so v1 and v2 readers agree."""
        data = self.model_dump()
        data["storedSize"] = data.get("storedSize") or self.compressedSize or self.size
        data["compressedSize"] = data["storedSize"]
        return data


class MediaUploadMeta(BaseModel):
    """Optional metadata accepted by POST /api/v1/media/upload."""

    sender: str | None = None
    sessionId: str | None = None
    conversationId: str | None = None
    recipient: str | None = None
    generateThumbnail: bool = True


def _host_allowed(url: str) -> bool:
    """Optional allowlist (``ADTP_MEDIA_ALLOWED_HOSTS``, comma separated).

    An empty allowlist keeps the existing trusted-relay behaviour. Entries may
    be exact hosts or ``.example.com`` to include subdomains.
    """
    from adtp_api.config import settings

    configured = (getattr(settings, "media_allowed_hosts", "") or "").strip()
    if not configured:
        return True
    host = (urlsplit(url).hostname or "").lower()
    for raw in configured.split(","):
        entry = raw.strip().lower()
        if not entry:
            continue
        if entry.startswith("."):
            if host.endswith(entry) and host != entry[1:]:
                return True
        elif host == entry:
            return True
    return False


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
    if not _host_allowed(service_url):
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
