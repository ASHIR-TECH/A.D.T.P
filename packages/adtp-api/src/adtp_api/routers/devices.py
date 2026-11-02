from __future__ import annotations

import aiosqlite
from fastapi import APIRouter, Depends, File, Query, Request, UploadFile
from fastapi.responses import FileResponse

from adtp_api.dependencies import get_db
from adtp_api.models.device import (
    DeviceFile,
    DeviceFileList,
    DeviceFileQueued,
    DeviceList,
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
