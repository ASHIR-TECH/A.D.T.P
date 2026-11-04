from __future__ import annotations

import aiosqlite
from fastapi import APIRouter, Depends, File, Query, Request, UploadFile
from fastapi.responses import FileResponse

from adtp_api.dependencies import get_db
from adtp_api.models.device import (
    DeviceFileList,
    DeviceFileQueued,
    DeviceList,
    DeviceMessageCreate,
    DeviceMessageList,
    DeviceMessageQueued,
    DeviceRecord,
    DeviceRegistration,
    DeviceRegistrationResult,
    DeviceTelemetry,
)
from adtp_api.services import device_service

router = APIRouter(prefix="/api/v1/devices", tags=["devices"])


@router.post("/register", status_code=201, response_model=DeviceRegistrationResult)
async def register_device(
    body: DeviceRegistration,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    """Called by MARS immediately after scanning the pairing QR.

    This is the missing half of pairing: the phone announces itself, so the
    desktop can list it, show its telemetry and queue files for it.
    """
    record, already_paired = await device_service.register(db, body)
    return {"device": record, "already_paired": already_paired}


@router.get("", response_model=DeviceList)
async def list_devices(
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> DeviceList:
    devices = await device_service.list_devices(db)
    return DeviceList(devices=devices, total=len(devices))


@router.get("/{device_id}", response_model=DeviceRecord)
async def get_device(
    device_id: str,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> DeviceRecord:
    return await device_service.get_device(db, device_id)


@router.post("/{device_id}/heartbeat", response_model=DeviceRecord)
async def heartbeat(
    device_id: str,
    body: DeviceTelemetry,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> DeviceRecord:
    return await device_service.heartbeat(db, device_id, body)


@router.delete("/{device_id}")
async def remove_device(
    device_id: str,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    await device_service.remove_device(db, device_id)
    return {"removed": True, "device_id": device_id}


@router.get("/{device_id}/files", response_model=DeviceFileList)
async def list_device_files(
    device_id: str,
    request: Request,
    include_delivered: bool = Query(False),
    db: aiosqlite.Connection = Depends(get_db),
) -> DeviceFileList:
    """Inbound inbox: files the desktop staged for this phone."""
    return await device_service.list_files(db, device_id, include_delivered)


@router.post("/{device_id}/files", status_code=201, response_model=DeviceFileQueued)
async def queue_device_file(
    device_id: str,
    request: Request,
    file: UploadFile = File(...),
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    """Stage a desktop file for one paired phone (desktop → phone)."""
    record = await device_service.queue_file(db, device_id, file)
    return {"file": record}


@router.get("/{device_id}/files/{file_id}")
async def download_device_file(
    device_id: str,
    file_id: str,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
):
    record, path = await device_service.open_file(db, device_id, file_id)
    return FileResponse(
        path=str(path),
        filename=record.filename,
        media_type=record.mime_type or "application/octet-stream",
    )


@router.post("/{device_id}/messages", status_code=201, response_model=DeviceMessageQueued)
async def queue_device_message(
    device_id: str,
    body: DeviceMessageCreate,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    """Queue a desktop → phone message and push it to a live socket.

    The durable row is the source of truth: if the phone is offline the gateway
    keeps the message until the phone polls its inbox.
    """
    record = await device_service.queue_message(db, device_id, body.text)
    from adtp_api.relay import notify_device

    await notify_device(
        device_id,
        {"type": "device_message", "message": record.model_dump()},
    )
    return {"message": record}


@router.get("/{device_id}/messages", response_model=DeviceMessageList)
async def list_device_messages(
    device_id: str,
    request: Request,
    include_delivered: bool = Query(False),
    db: aiosqlite.Connection = Depends(get_db),
) -> DeviceMessageList:
    return await device_service.list_messages(db, device_id, include_delivered)


@router.post("/{device_id}/messages/{message_id}/delivered")
async def mark_device_message_delivered(
    device_id: str,
    message_id: str,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    await device_service.mark_message_delivered(db, device_id, message_id)
    return {"delivered": True, "message_id": message_id}


@router.post("/{device_id}/ping")
async def ping_device(
    device_id: str,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    """Presence probe: true only while the phone's relay socket is attached."""
    await device_service.get_device(db, device_id)
    from adtp_api.relay import notify_device

    sent = await notify_device(device_id, {"type": "ping"})
    return {"sent": sent, "online": sent > 0, "device_id": device_id}
