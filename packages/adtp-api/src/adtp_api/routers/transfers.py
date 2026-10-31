from __future__ import annotations

from fastapi import APIRouter, Depends, File, HTTPException, Query, Request, UploadFile
from fastapi.responses import FileResponse
from slowapi import Limiter
from slowapi.util import get_remote_address

import aiosqlite
from pathlib import Path

from adtp_api.dependencies import get_db
from adtp_api.models.transfer import TransferRequest, TransferList, UploadResponse
from adtp_api.services import transfer_service
from adtp_api.services.system_service import resolved_intake_dir

router = APIRouter(prefix="/api/v1/transfers", tags=["transfers"])
limiter = Limiter(key_func=get_remote_address)


@router.post("/send", status_code=201)
@limiter.limit("10/minute")
async def send_file(
    body: TransferRequest, request, db: aiosqlite.Connection = Depends(get_db)
):
    result = await transfer_service.send_file(
        body.file_path, body.agent_name, body.priority, body.message
    )
    return result


@router.post("/upload", status_code=201, response_model=UploadResponse)
@limiter.limit("60/minute")
async def upload_file(
    request: Request,
    file: UploadFile = File(...),
    db: aiosqlite.Connection = Depends(get_db),
):
    """Accept a file from the paired phone and persist it to the intake dir.

    Unlike /send (which only forwards a file already on this machine), this is
    the inbound path for the phone's attachment. The response carries the
    absolute desktop path, which the phone appends to the agent chat so the
    desktop AI can act on the uploaded file.
    """
    return await transfer_service.receive_upload(file)


@router.get("/{transfer_id}/download")
async def download_file(
    transfer_id: str,
    db: aiosqlite.Connection = Depends(get_db),
):
    """Download a transferred file by transfer ID.

    For uploads (phone → desktop), the file is in the intake directory.
    For incoming transfers (desktop ← desktop), the file is in a watch folder.
    This endpoint attempts to locate and serve the file.
    """
    # Look up the transfer to get filename
    transfer = await transfer_service.get_transfer(db, transfer_id)
    filename = transfer.filename

    # Try intake directory first (uploads from phone)
    intake_dir = resolved_intake_dir()
    intake_path = intake_dir / filename
    if intake_path.exists():
        return FileResponse(
            path=str(intake_path),
            filename=filename,
            media_type=transfer.mime_type or "application/octet-stream",
        )

    # TODO: For incoming QUIC transfers, check watch folders
    # This would require tracking file_path in transfer_audit or querying watchers

    raise HTTPException(status_code=404, detail="File not found on this desktop")


@router.post("/push", status_code=201)
async def push_file(body: TransferRequest, db: aiosqlite.Connection = Depends(get_db)):
    return await transfer_service.push_file(body.file_path)


@router.get("")
async def list_transfers(
    status: str = Query("all", pattern="^(pending|delivered|failed|all)$"),
    limit: int = Query(20, ge=1, le=100),
    offset: int = Query(0, ge=0),
    session_id: str | None = Query(None),
    peer_id: str | None = Query(None),
    db: aiosqlite.Connection = Depends(get_db),
) -> TransferList:
    return await transfer_service.list_transfers(db, status, limit, offset, session_id, peer_id)


@router.get("/{transfer_id}")
async def get_transfer(transfer_id: str, db: aiosqlite.Connection = Depends(get_db)):
    return await transfer_service.get_transfer(db, transfer_id)


@router.delete("/{transfer_id}")
async def cancel_transfer(transfer_id: str):
    result = await transfer_service.cancel_transfer(transfer_id)
    return {"cancelled": result}


@router.get("/{transfer_id}/status")
async def get_status(transfer_id: str):
    return await transfer_service.get_transfer_status(transfer_id)


@router.get("/{transfer_id}/context")
async def get_context(transfer_id: str, db: aiosqlite.Connection = Depends(get_db)):
    context = await transfer_service.get_transfer_context(db, transfer_id)
    if context is None:
        return {
            "schema": None,
            "summary": None,
            "agent_hint": None,
            "transfer_history": [],
            "extraction_ms": None,
        }
    return context
