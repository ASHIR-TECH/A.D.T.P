from __future__ import annotations

import hashlib
import json
import os
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

import aiosqlite

from adtp_api.config import settings, default_data_dir
from adtp_api.errors import DeviceNotFoundError, FileTooLargeError
from adtp_api.mime_types import guess_mime
from adtp_api.models.device import (
    DeviceFile,
    DeviceFileList,
    DeviceMessage,
    DeviceMessageList,
    DeviceRecord,
    DeviceRegistration,
    DeviceTelemetry,
)

# A device is reported online while its last heartbeat is this fresh. MARS
# heartbeats every 30 s, so a 90 s window tolerates one dropped beat.
ONLINE_WINDOW = timedelta(seconds=90)

DEVICES_SCHEMA = """CREATE TABLE IF NOT EXISTS paired_devices (
    device_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    os_name TEXT NOT NULL DEFAULT '',
    os_version TEXT NOT NULL DEFAULT '',
    app_version TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    battery_percent REAL,
    battery_charging INTEGER,
    capabilities TEXT NOT NULL DEFAULT '[]',
    paired_at TEXT NOT NULL,
    last_seen TEXT NOT NULL
)"""

DEVICE_FILES_SCHEMA = """CREATE TABLE IF NOT EXISTS device_files (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    stored_name TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    mime_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    sha256 TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    delivered_at TEXT
)"""

# Desktop → phone text messages. Kept small and durable so a phone that is
# offline when the desktop sends a message still receives it on reconnect.
DEVICE_MESSAGES_SCHEMA = """CREATE TABLE IF NOT EXISTS device_messages (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    delivered_at TEXT
)"""


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


async def ensure_device_tables(db: aiosqlite.Connection) -> None:
    await db.execute(DEVICES_SCHEMA)
    await db.execute(DEVICE_FILES_SCHEMA)
    await db.execute(DEVICE_MESSAGES_SCHEMA)
    await db.execute(
        "CREATE INDEX IF NOT EXISTS idx_device_files_device "
        "ON device_files (device_id, created_at)"
    )
    await db.execute(
        "CREATE INDEX IF NOT EXISTS idx_device_messages_device "
        "ON device_messages (device_id, created_at)"
    )
    await db.commit()


def resolved_outbox_dir(device_id: str | None = None) -> Path:
    """Where desktop → phone files are staged (`<data_dir>/adtp/outbox`)."""
    override = os.getenv("ADTP_OUTBOX_DIR") or getattr(settings, "outbox_dir", "")
    base = Path(override) if override else default_data_dir() / "adtp" / "outbox"
    return base / device_id if device_id else base


def _safe_device_dir(device_id: str) -> str:
    # device_id is validated at the model boundary; keep the on-disk name
    # defensive anyway so a bad value can never escape the outbox.
    return "".join(ch for ch in device_id if ch.isalnum() or ch in "-_")[:128] or "device"


def _safe_filename(name: str | None) -> str:
    cleaned = Path(name or "").name.strip()
    if not cleaned or cleaned in {".", ".."}:
        return "file.bin"
    return cleaned


def _unique_destination(directory: Path, filename: str) -> Path:
    candidate = directory / filename
    if not candidate.exists():
        return candidate
    stem, suffix = candidate.stem, candidate.suffix
    for index in range(1, 10_000):
        alternate = directory / f"{stem}-{index}{suffix}"
        if not alternate.exists():
            return alternate
    return directory / f"{uuid.uuid4().hex}-{filename}"


def _is_online(last_seen: str) -> bool:
    try:
        seen = datetime.fromisoformat(last_seen)
    except (TypeError, ValueError):
        return False
    if seen.tzinfo is None:
        seen = seen.replace(tzinfo=timezone.utc)
    return datetime.now(timezone.utc) - seen <= ONLINE_WINDOW


def _row_to_device(row: aiosqlite.Row) -> DeviceRecord:
    try:
        capabilities = json.loads(row["capabilities"] or "[]")
    except (json.JSONDecodeError, TypeError):
        capabilities = []
    if not isinstance(capabilities, list):
        capabilities = []
    charging = row["battery_charging"]
    return DeviceRecord(
        device_id=row["device_id"],
        name=row["name"],
        platform=row["platform"],
        os_name=row["os_name"] or "",
        os_version=row["os_version"] or "",
        app_version=row["app_version"] or "",
        model=row["model"] or "",
        battery_percent=row["battery_percent"],
        battery_charging=None if charging is None else bool(charging),
        capabilities=capabilities,
        paired_at=row["paired_at"],
        last_seen=row["last_seen"],
        online=_is_online(row["last_seen"]),
    )


def _row_to_file(row: aiosqlite.Row) -> DeviceFile:
    return DeviceFile(
        id=row["id"],
        device_id=row["device_id"],
        filename=row["filename"],
        file_size=int(row["file_size"] or 0),
        mime_type=row["mime_type"] or "application/octet-stream",
        sha256=row["sha256"] or "",
        created_at=row["created_at"],
        delivered_at=row["delivered_at"],
    )


def _row_to_message(row: aiosqlite.Row) -> DeviceMessage:
    return DeviceMessage(
        id=row["id"],
        device_id=row["device_id"],
        text=row["text"],
        created_at=row["created_at"],
        delivered_at=row["delivered_at"],
    )


async def register(
    db: aiosqlite.Connection, device: DeviceRegistration
) -> tuple[DeviceRecord, bool]:
    """Upsert a phone. Returns (record, already_paired).

    Calling register again is how a phone re-announces itself after a restart,
    so it must be idempotent and refresh telemetry + last_seen.
    """
    existing = await db.execute_fetchall(
        "SELECT device_id FROM paired_devices WHERE device_id = ?", [device.device_id]
    )
    already_paired = bool(existing)
    now = _now()
    capabilities = json.dumps(device.capabilities)
    if already_paired:
        await db.execute(
            "UPDATE paired_devices SET name = ?, platform = ?, os_name = ?, "
            "os_version = ?, app_version = ?, model = ?, battery_percent = ?, "
            "battery_charging = ?, capabilities = ?, last_seen = ? "
            "WHERE device_id = ?",
            (
                device.name,
                device.platform,
                device.os_name,
                device.os_version,
                device.app_version,
                device.model,
                device.battery_percent,
                None
                if device.battery_charging is None
                else int(device.battery_charging),
                capabilities,
                now,
                device.device_id,
            ),
        )
    else:
        await db.execute(
            "INSERT INTO paired_devices (device_id, name, platform, os_name, "
            "os_version, app_version, model, battery_percent, battery_charging, "
            "capabilities, paired_at, last_seen) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                device.device_id,
                device.name,
                device.platform,
                device.os_name,
                device.os_version,
                device.app_version,
                device.model,
                device.battery_percent,
                None
                if device.battery_charging is None
                else int(device.battery_charging),
                capabilities,
                now,
                now,
            ),
        )
    await db.commit()
    record = await get_device(db, device.device_id)
    return record, already_paired


async def heartbeat(
    db: aiosqlite.Connection, device_id: str, telemetry: DeviceTelemetry
) -> DeviceRecord:
    await get_device(db, device_id)  # 404 when unknown
    updates: list[str] = ["last_seen = ?"]
    params: list = [_now()]

    simple_fields = (
        "name",
        "platform",
        "os_name",
        "os_version",
        "app_version",
        "model",
    )
    for field in simple_fields:
        value = getattr(telemetry, field)
        if value is not None:
            updates.append(f"{field} = ?")
            params.append(value)
    if telemetry.battery_percent is not None:
        updates.append("battery_percent = ?")
        params.append(telemetry.battery_percent)
    if telemetry.battery_charging is not None:
        updates.append("battery_charging = ?")
        params.append(int(telemetry.battery_charging))
    if telemetry.capabilities is not None:
        updates.append("capabilities = ?")
        params.append(json.dumps(telemetry.capabilities))

    params.append(device_id)
    await db.execute(
        f"UPDATE paired_devices SET {', '.join(updates)} WHERE device_id = ?", params
    )
    await db.commit()
    return await get_device(db, device_id)


async def touch(db: aiosqlite.Connection, device_id: str) -> None:
    """Refresh `last_seen` without a full heartbeat.

    The relay calls this when a phone's WebSocket authenticates, so the desktop
    shows the phone as online immediately instead of waiting up to 20 s for the
    next REST heartbeat. Unknown ids are ignored (a stale pairing must not
    resurrect a device the user removed).
    """
    await db.execute(
        "UPDATE paired_devices SET last_seen = ? WHERE device_id = ?",
        (_now(), device_id),
    )
    await db.commit()


async def get_device(db: aiosqlite.Connection, device_id: str) -> DeviceRecord:
    rows = await db.execute_fetchall(
        "SELECT * FROM paired_devices WHERE device_id = ?", [device_id]
    )
    if not rows:
        raise DeviceNotFoundError(device_id)
    return _row_to_device(rows[0])


async def list_devices(db: aiosqlite.Connection) -> list[DeviceRecord]:
    rows = await db.execute_fetchall(
        "SELECT * FROM paired_devices ORDER BY last_seen DESC"
    )
    return [_row_to_device(row) for row in rows]


async def remove_device(db: aiosqlite.Connection, device_id: str) -> bool:
    await get_device(db, device_id)
    await db.execute("DELETE FROM device_files WHERE device_id = ?", [device_id])
    await db.execute("DELETE FROM device_messages WHERE device_id = ?", [device_id])
    await db.execute("DELETE FROM paired_devices WHERE device_id = ?", [device_id])
    await db.commit()

    directory = resolved_outbox_dir(_safe_device_dir(device_id))
    if directory.exists():
        for entry in directory.iterdir():
            try:
                entry.unlink()
            except OSError:
                pass
        try:
            directory.rmdir()
        except OSError:
            pass
    return True


async def queue_file(
    db: aiosqlite.Connection, device_id: str, upload
) -> DeviceFile:
    """Stage a desktop file for one phone and record it in the inbox."""
    await get_device(db, device_id)
    directory = resolved_outbox_dir(_safe_device_dir(device_id))
    directory.mkdir(parents=True, exist_ok=True)

    destination = _unique_destination(directory, _safe_filename(upload.filename))
    hasher = hashlib.sha256()
    size = 0
    try:
        with destination.open("wb") as out:
            while True:
                chunk = await upload.read(1024 * 1024)
                if not chunk:
                    break
                size += len(chunk)
                if size > settings.max_file_size:
                    raise FileTooLargeError(settings.max_file_size)
                hasher.update(chunk)
                out.write(chunk)
    except Exception:
        destination.unlink(missing_ok=True)
        raise
    finally:
        await upload.close()

    file_id = uuid.uuid4().hex
    record = DeviceFile(
        id=file_id,
        device_id=device_id,
        filename=destination.name,
        file_size=size,
        mime_type=guess_mime(destination.name, upload.content_type),
        sha256=hasher.hexdigest(),
        created_at=_now(),
    )
    await db.execute(
        "INSERT INTO device_files (id, device_id, filename, stored_name, file_size, "
        "mime_type, sha256, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            record.id,
            device_id,
            record.filename,
            destination.name,
            size,
            record.mime_type,
            record.sha256,
            record.created_at,
        ),
    )
    await db.commit()
    return record


async def list_files(
    db: aiosqlite.Connection, device_id: str, include_delivered: bool = False
) -> DeviceFileList:
    await get_device(db, device_id)
    where = "" if include_delivered else "AND delivered_at IS NULL"
    rows = await db.execute_fetchall(
        f"SELECT * FROM device_files WHERE device_id = ? {where} "
        "ORDER BY created_at ASC",
        [device_id],
    )
    files = [_row_to_file(row) for row in rows]
    return DeviceFileList(files=files, total=len(files))


async def open_file(
    db: aiosqlite.Connection, device_id: str, file_id: str
) -> tuple[DeviceFile, Path]:
    rows = await db.execute_fetchall(
        "SELECT * FROM device_files WHERE id = ? AND device_id = ?",
        [file_id, device_id],
    )
    if not rows:
        raise DeviceNotFoundError(f"{device_id}/{file_id}")
    record = _row_to_file(rows[0])
    path = resolved_outbox_dir(_safe_device_dir(device_id)) / rows[0]["stored_name"]
    if not path.exists():
        raise DeviceNotFoundError(f"file {record.filename}")

    await db.execute(
        "UPDATE device_files SET delivered_at = ? WHERE id = ?", [_now(), file_id]
    )
    await db.commit()
    return record, path


async def queue_message(
    db: aiosqlite.Connection, device_id: str, text: str
) -> DeviceMessage:
    """Store a desktop → phone message, then return it for live delivery."""
    await get_device(db, device_id)
    record = DeviceMessage(
        id=uuid.uuid4().hex,
        device_id=device_id,
        text=text.strip(),
        created_at=_now(),
    )
    await db.execute(
        "INSERT INTO device_messages (id, device_id, text, created_at) "
        "VALUES (?, ?, ?, ?)",
        (record.id, device_id, record.text, record.created_at),
    )
    await db.commit()
    return record


async def list_messages(
    db: aiosqlite.Connection, device_id: str, include_delivered: bool = False
) -> DeviceMessageList:
    await get_device(db, device_id)
    where = "" if include_delivered else "AND delivered_at IS NULL"
    rows = await db.execute_fetchall(
        f"SELECT * FROM device_messages WHERE device_id = ? {where} "
        "ORDER BY created_at ASC",
        [device_id],
    )
    messages = [_row_to_message(row) for row in rows]
    return DeviceMessageList(messages=messages, total=len(messages))


async def mark_message_delivered(
    db: aiosqlite.Connection, device_id: str, message_id: str
) -> None:
    await db.execute(
        "UPDATE device_messages SET delivered_at = ? "
        "WHERE id = ? AND device_id = ? AND delivered_at IS NULL",
        (_now(), message_id, device_id),
    )
    await db.commit()
