from __future__ import annotations

from datetime import datetime, timezone

from pydantic import BaseModel, Field, field_validator

# A paired phone is identified by a stable install id created by the mobile app.
# Keep the charset tight: the id is also used as an on-disk folder name.
_DEVICE_ID_ALLOWED = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_")


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

    @field_validator("device_id")
    @classmethod
    def _validate_device_id(cls, value: str) -> str:
        cleaned = value.strip()
        if not cleaned or any(ch not in _DEVICE_ID_ALLOWED for ch in cleaned):
            raise ValueError(
                "device_id must contain only letters, digits, '-' and '_'"
            )
        return cleaned

    @field_validator("capabilities")
    @classmethod
    def _capabilities(cls, value: list[str]) -> list[str]:
        return [item.strip()[:40] for item in value if item and item.strip()][:32]


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
