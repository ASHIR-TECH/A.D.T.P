from __future__ import annotations

import math
from datetime import datetime, timezone
from typing import Any

from pydantic import BaseModel, Field, ValidationInfo, field_validator

# A paired phone is identified by a stable install id created by the mobile app.
# The id is also used as a folder name, so path separators and traversal are
# rejected. Everything else is allowed: phones in the wild send UUIDs, install
# ids with punctuation, and occasional stray whitespace.
_FORBIDDEN_DEVICE_ID = ("/", "\\", "..")

_TEXT_LIMITS = {
    "name": 120,
    "platform": 40,
    "os_name": 80,
    "os_version": 80,
    "app_version": 40,
    "model": 120,
}

_TEXT_DEFAULTS = {
    "name": "MARS",
    "platform": "mobile",
    "os_name": "",
    "os_version": "",
    "app_version": "",
    "model": "",
}


def _text(value: Any, default: str = "") -> str:
    """Coerce phone-supplied telemetry into a clean, bounded string.

    Pairing must not fail because a build sent a number where a string was
    expected, or a multi-line OS build identifier. Every string field goes
    through here instead of letting pydantic return a bare 422.
    """
    if value is None:
        return default
    if isinstance(value, bool):
        return "true" if value else "false"
    text = " ".join(str(value).split())
    return text or default


def _battery_percent(value: Any) -> float | None:
    """Accept 0-100 percentages, 0-1 levels, numeric strings, and junk.

    Unusable readings become ``None`` rather than a validation error: battery
    telemetry is decoration, not a pairing requirement.
    """
    if value is None or isinstance(value, bool):
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(number) or number < 0 or number > 100:
        return None
    # Some builds reported the raw expo-battery level (0.0-1.0) instead of a
    # percentage. Leave exact 1.0 alone so a real 1% reading stays 1%.
    if 0 < number < 1:
        number *= 100
    return round(number, 1)


def _battery_charging(value: Any) -> bool | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return bool(value) if value in (0, 1) else None
    if isinstance(value, str):
        lowered = value.strip().lower()
        if lowered in {"true", "yes", "1", "charging", "full", "plugged"}:
            return True
        if lowered in {
            "false",
            "no",
            "0",
            "unplugged",
            "discharging",
            "not charging",
        }:
            return False
    return None


def _capabilities_list(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        items: list[Any] = value.split(",")
    elif isinstance(value, (list, tuple, set)):
        items = list(value)
    else:
        return []
    cleaned: list[str] = []
    for item in items:
        text = _text(item)[:40]
        if text:
            cleaned.append(text)
        if len(cleaned) >= 32:
            break
    return cleaned


class DeviceRegistration(BaseModel):
    """Identity + telemetry a paired phone hands to the desktop gateway."""

    device_id: str = Field(min_length=6, max_length=128)
    name: str = Field(default="MARS", max_length=120)
    platform: str = Field(default="mobile", max_length=40)
    os_name: str = Field(default="", max_length=80)
    os_version: str = Field(default="", max_length=80)
    app_version: str = Field(default="", max_length=40)
    model: str = Field(default="", max_length=120)
    battery_percent: float | None = Field(default=None, ge=0, le=100)
    battery_charging: bool | None = None
    capabilities: list[str] = Field(default_factory=list)

    @field_validator("device_id", mode="before")
    @classmethod
    def _coerce_device_id(cls, value: Any) -> str:
        return "" if value is None else str(value).strip()

    @field_validator(
        "name",
        "platform",
        "os_name",
        "os_version",
        "app_version",
        "model",
        mode="before",
    )
    @classmethod
    def _coerce_text(cls, value: Any, info: ValidationInfo) -> str:
        field = info.field_name or ""
        return _text(value, _TEXT_DEFAULTS.get(field, ""))[: _TEXT_LIMITS.get(field, 120)]

    @field_validator("battery_percent", mode="before")
    @classmethod
    def _coerce_battery_percent(cls, value: Any) -> float | None:
        return _battery_percent(value)

    @field_validator("battery_charging", mode="before")
    @classmethod
    def _coerce_battery_charging(cls, value: Any) -> bool | None:
        return _battery_charging(value)

    @field_validator("capabilities", mode="before")
    @classmethod
    def _coerce_capabilities(cls, value: Any) -> list[str]:
        return _capabilities_list(value)

    @field_validator("device_id")
    @classmethod
    def _validate_device_id(cls, value: str) -> str:
        cleaned = value.strip()
        if not cleaned:
            raise ValueError("device_id is required")
        if any(marker in cleaned for marker in _FORBIDDEN_DEVICE_ID):
            raise ValueError("device_id must not contain path separators or '..'")
        if any(ord(ch) < 32 for ch in cleaned):
            raise ValueError("device_id must not contain control characters")
        return cleaned[:128]


class DeviceTelemetry(BaseModel):
    """Mutable part of a paired device record (heartbeat / status updates)."""

    name: str | None = Field(default=None, max_length=120)
    platform: str | None = Field(default=None, max_length=40)
    os_name: str | None = Field(default=None, max_length=80)
    os_version: str | None = Field(default=None, max_length=80)
    app_version: str | None = Field(default=None, max_length=40)
    model: str | None = Field(default=None, max_length=120)
    battery_percent: float | None = Field(default=None, ge=0, le=100)
    battery_charging: bool | None = None
    capabilities: list[str] | None = None

    @field_validator(
        "name",
        "platform",
        "os_name",
        "os_version",
        "app_version",
        "model",
        mode="before",
    )
    @classmethod
    def _coerce_optional_text(cls, value: Any, info: ValidationInfo) -> str | None:
        if value is None:
            return None
        field = info.field_name or ""
        text = _text(value)[: _TEXT_LIMITS.get(field, 120)]
        return text or None

    @field_validator("battery_percent", mode="before")
    @classmethod
    def _coerce_battery_percent(cls, value: Any) -> float | None:
        return _battery_percent(value)

    @field_validator("battery_charging", mode="before")
    @classmethod
    def _coerce_battery_charging(cls, value: Any) -> bool | None:
        return _battery_charging(value)

    @field_validator("capabilities", mode="before")
    @classmethod
    def _coerce_capabilities(cls, value: Any) -> list[str] | None:
        if value is None:
            return None
        return _capabilities_list(value)


class DeviceRecord(BaseModel):
    device_id: str
    name: str
    platform: str
    os_name: str = ""
    os_version: str = ""
    app_version: str = ""
    model: str = ""
    battery_percent: float | None = None
    battery_charging: bool | None = None
    capabilities: list[str] = Field(default_factory=list)
    paired_at: str
    last_seen: str
    online: bool = False

    @staticmethod
    def now() -> str:
        return datetime.now(timezone.utc).isoformat()


class DeviceList(BaseModel):
    devices: list[DeviceRecord]
    total: int


class DeviceFile(BaseModel):
    """A file queued on the desktop for a specific phone."""

    id: str
    device_id: str
    filename: str
    file_size: int
    mime_type: str = "application/octet-stream"
    sha256: str = ""
    created_at: str
    delivered_at: str | None = None


class DeviceFileList(BaseModel):
    files: list[DeviceFile]
    total: int


class DeviceFileQueued(BaseModel):
    file: DeviceFile


class DeviceMessageCreate(BaseModel):
    """Desktop → phone text (chat reply, notification, AI note)."""

    text: str = Field(min_length=1, max_length=4000)


class DeviceMessage(BaseModel):
    id: str
    device_id: str
    text: str
    created_at: str
    delivered_at: str | None = None


class DeviceMessageList(BaseModel):
    messages: list[DeviceMessage]
    total: int


class DeviceMessageQueued(BaseModel):
    message: DeviceMessage


class DeviceRegistrationResult(BaseModel):
    device: DeviceRecord
    already_paired: bool = False
