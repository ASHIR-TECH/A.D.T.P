from __future__ import annotations

from pydantic import BaseModel


class SystemAgentInfo(BaseModel):
    """Snapshot of the desktop's embedded agent (mirrors /agent/status)."""

    active: bool = False
    provider: str | None = None
    model: str | None = None


class SystemInfo(BaseModel):
    """Host telemetry for the paired desktop.

    Every metric is optional: the gateway degrades to null when psutil is
    unavailable and the platform doesn't expose an equivalent, rather than
    failing the whole response. The mobile app shows the pairing-time fields
    (name/os/arch) and only uses the metrics when present.
    """

    hostname: str
    os: str
    os_version: str = ""
    arch: str = ""
    cpu_count: int | None = None
    cpu_percent: float | None = None
    ram_total: int | None = None
    ram_used: int | None = None
    ram_percent: float | None = None
    disk_total: int | None = None
    disk_used: int | None = None
    disk_percent: float | None = None
    uptime_seconds: float | None = None
    primary_ip: str | None = None
    python_version: str = ""
    gateway_version: str = "0.1.0"
    port: int = 40003
    intake_dir: str = ""
    agent: SystemAgentInfo = SystemAgentInfo()
