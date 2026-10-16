from __future__ import annotations

import os
import platform
import shutil
import socket
import time
from pathlib import Path

from adtp_api.config import settings, default_intake_dir


def resolved_intake_dir() -> Path:
    """The folder uploads are written to (config override or shared default)."""
    override = os.getenv("ADTP_INTAKE_DIR") or settings.intake_dir
    return Path(override) if override else default_intake_dir()


def _as_bytes(value: str | None) -> int | None:
    if not value:
        return None
    token = value.split()[0]
    try:
        return int(token) * 1024  # /proc/meminfo reports kibibytes
    except ValueError:
        return None


def _ram() -> tuple[int | None, int | None]:
    """(total, used) bytes. psutil first, then native per-OS fallbacks."""
    try:
        import psutil  # type: ignore[import-not-found]

        vm = psutil.virtual_memory()
        return int(vm.total), int(vm.used)
    except Exception:
        pass

    system = platform.system()
    try:
        if system == "Linux":
            values: dict[str, str] = {}
            with open("/proc/meminfo", "r", encoding="utf-8") as fh:
                for line in fh:
                    key, _, rest = line.partition(":")
                    values[key.strip()] = rest.strip()
            total = _as_bytes(values.get("MemTotal"))
            available = _as_bytes(values.get("MemAvailable")) or _as_bytes(
                values.get("MemFree")
            )
            if total:
                used = total - (available or 0)
                return total, max(used, 0)
        elif system == "Darwin":
            import subprocess

            total = int(
                subprocess.check_output(["sysctl", "-n", "hw.memsize"]).strip()
            )
            return total, None
        elif system == "Windows":
            import ctypes

            class MemoryStatusEx(ctypes.Structure):
                _fields_ = [
                    ("dwLength", ctypes.c_ulong),
                    ("dwMemoryLoad", ctypes.c_ulong),
                    ("ullTotalPhys", ctypes.c_ulonglong),
                    ("ullAvailPhys", ctypes.c_ulonglong),
                    ("ullTotalPageFile", ctypes.c_ulonglong),
                    ("ullAvailPageFile", ctypes.c_ulonglong),
                    ("ullTotalVirtual", ctypes.c_ulonglong),
                    ("ullAvailVirtual", ctypes.c_ulonglong),
                    ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
                ]

            status = MemoryStatusEx()
            status.dwLength = ctypes.sizeof(MemoryStatusEx)
            ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status))
            return int(status.ullTotalPhys), int(
                status.ullTotalPhys - status.ullAvailPhys
            )
    except Exception:
        pass
    return None, None


def _cpu_percent() -> float | None:
    try:
        import psutil  # type: ignore[import-not-found]

        return float(psutil.cpu_percent(interval=None))
    except Exception:
        pass
    try:
        load = os.getloadavg()[0]
        count = os.cpu_count() or 1
        return round(min(load / count * 100.0, 100.0), 1)
    except (AttributeError, OSError):
        return None


def _uptime_seconds() -> float | None:
    try:
        import psutil  # type: ignore[import-not-found]

        return max(time.time() - psutil.boot_time(), 0.0)
    except Exception:
        pass
    try:
        if platform.system() == "Linux":
            with open("/proc/uptime", "r", encoding="utf-8") as fh:
                return float(fh.read().split()[0])
        if platform.system() == "Windows":
            import ctypes

            return ctypes.windll.kernel32.GetTickCount64() / 1000.0
    except Exception:
        pass
    return None


def _primary_ip() -> str | None:
    """Best-effort LAN address. Opens a UDP socket — sends no packets."""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            sock.connect(("8.8.8.8", 80))
            return sock.getsockname()[0]
        finally:
            sock.close()
    except Exception:
        return None


def identity() -> dict:
    """Cheap machine identity for relay `state_update` frames (no disk probing)."""
    return {
        "hostname": platform.node() or "Desktop",
        "os": platform.system() or "Desktop",
    }


async def gather(
    db, agent_socket: str, daemon_socket: str, port: int | None = None
) -> dict:
    """Assemble the /api/v1/system/info payload."""
    from adtp_api.services import agent_service

    total_ram, used_ram = _ram()
    disk_total = disk_used = disk_percent = None
    try:
        usage = shutil.disk_usage(str(Path.home()))
        disk_total, disk_used = usage.total, usage.used
        disk_percent = round(usage.used / usage.total * 100, 1) if usage.total else None
    except Exception:
        pass

    agent_status: dict = {"active": False, "provider": None, "model": None}
    try:
        agent_status = await agent_service.get_status(db, agent_socket, daemon_socket)
    except Exception:
        pass

    return {
        "hostname": platform.node() or "Desktop",
        "os": platform.system() or "Desktop",
        "os_version": f"{platform.release()} {platform.version()}".strip(),
        "arch": platform.machine(),
        "cpu_count": os.cpu_count(),
        "cpu_percent": _cpu_percent(),
        "ram_total": total_ram,
        "ram_used": used_ram,
        "ram_percent": (
            round(used_ram / total_ram * 100, 1) if total_ram and used_ram else None
        ),
        "disk_total": disk_total,
        "disk_used": disk_used,
        "disk_percent": disk_percent,
        "uptime_seconds": _uptime_seconds(),
        "primary_ip": _primary_ip(),
        "python_version": platform.python_version(),
        "gateway_version": "0.1.0",
        "port": port if port is not None else settings.api_port,
        "intake_dir": str(resolved_intake_dir()),
        "agent": {
            "active": bool(agent_status.get("active", False)),
            "provider": agent_status.get("provider"),
            "model": agent_status.get("model"),
        },
    }
