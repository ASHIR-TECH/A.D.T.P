"""Media REST API: compress + store in the shared Upstash Blob bucket.

The desktop UI and the agent use this before sending media. The returned
MediaRef is the only thing put into chat/device messages; bytes never are.
"""
from __future__ import annotations

from fastapi import APIRouter, Depends, File, Form, Request, UploadFile
from fastapi.responses import RedirectResponse

from adtp_api.dependencies import get_db
from adtp_api.models.media import MediaRef
from adtp_api.services import media_service

router = APIRouter(prefix="/api/v1/media", tags=["media"])


@router.post("/upload", response_model=MediaRef, status_code=201)
async def upload_media(
    request: Request,
    file: UploadFile = File(...),
    sender: str | None = Form(None),
    sessionId: str | None = Form(None),
    generateThumbnail: bool = Form(True),
) -> MediaRef:
    """Accept a local file, compress it and store it in the blob bucket."""
    return await media_service.store_upload(
        file,
        sender=sender or "adtp-desktop",
        session_id=sessionId,
        generate_thumbnail=generateThumbnail,
    )


@router.get("/{media_id}", response_model=MediaRef)
async def get_media(media_id: str, request: Request) -> MediaRef:
    """Rehydrate a MediaRef from the relay (for old persisted messages)."""
    return await media_service.fetch(media_id)


@router.get("/{media_id}/file")
async def media_file(media_id: str, kind: str = "original", download: bool = False):
    """Redirect to a freshly signed read URL from the relay."""
    from urllib.parse import quote

    base = (media_service._relay_base())
    if not base:
        return RedirectResponse(url="/", status_code=404)
    query = f"kind={'thumbnail' if kind == 'thumbnail' else 'original'}"
    if download:
        query += "&download=1"
    return RedirectResponse(
        url=f"{base}/api/media/file/{quote(media_id, safe='')}?{query}", status_code=302
    )
