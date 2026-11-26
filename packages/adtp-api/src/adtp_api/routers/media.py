"""Media REST API: compress + store in the shared Upstash Blob bucket.

The desktop UI and the agent use this before sending media. The returned
MediaRef is the only thing put into chat/device messages; bytes never are.

Two read paths exist:
  * GET /api/v1/media/{id}/file  (authenticated, used by the desktop UI)
  * GET /api/media/file/{id}     (header-free, used by the phone's <Image>)
Both redirect to a freshly signed relay URL, so the bucket stays private.
"""
from __future__ import annotations

from urllib.parse import quote

from fastapi import APIRouter, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import RedirectResponse

from adtp_api.models.media import MediaRef
from adtp_api.services import media_service

router = APIRouter(prefix="/api/v1/media", tags=["media"])
# Header-free compat path that mirrors the MARS relay wire format. The id is a
# random UUID used as a capability, exactly like the relay's /api/media/file.
public_router = APIRouter(prefix="/api/media", tags=["media"])


def _gateway_base(request: Request) -> str:
    """Public base URL the caller used to reach this gateway.

    A phone paired over the LAN (or a tunnel) already has a working route here,
    but it may not be able to reach the relay's public URL directly. Returning
    this base makes the phone read media through the gateway, which proxies to
    the relay server-to-server.
    """
    forwarded_proto = request.headers.get("x-forwarded-proto", "").split(",")[0].strip()
    scheme = forwarded_proto or request.url.scheme
    host = (
        request.headers.get("x-forwarded-host")
        or request.headers.get("host")
        or request.url.netloc
    )
    return f"{scheme}://{host}".rstrip("/")


def _through_gateway(media: MediaRef, request: Request) -> MediaRef:
    """Point the client at this gateway instead of the relay."""
    media.serviceUrl = _gateway_base(request)
    return media


def _relay_redirect(media_id: str, kind: str, download: bool) -> RedirectResponse:
    """302 to a freshly signed relay URL (the relay mints it per request)."""
    base = media_service._relay_base()
    if not base:
        raise HTTPException(503, "Media storage is not configured (ADTP_RELAY_URL)")
    query = f"kind={'thumbnail' if kind == 'thumbnail' else 'original'}"
    if download:
        query += "&download=1"
    return RedirectResponse(
        url=f"{base}/api/media/file/{quote(media_id, safe='')}?{query}",
        status_code=302,
    )


@router.post("/upload", response_model=MediaRef, status_code=201)
async def upload_media(
    request: Request,
    file: UploadFile = File(...),
    sender: str | None = Form(None),
    sessionId: str | None = Form(None),
    conversationId: str | None = Form(None),
    recipient: str | None = Form(None),
    generateThumbnail: bool = Form(True),
) -> MediaRef:
    """Accept a local file, compress it and store it in the blob bucket."""
    media = await media_service.store_upload(
        file,
        sender=sender or "adtp-desktop",
        session_id=sessionId,
        conversation_id=conversationId,
        recipient=recipient,
        generate_thumbnail=generateThumbnail,
    )
    return _through_gateway(media, request)


@router.get("/{media_id}", response_model=MediaRef)
async def get_media(media_id: str, request: Request) -> MediaRef:
    """Rehydrate a MediaRef from the relay (for old persisted messages)."""
    return _through_gateway(await media_service.fetch(media_id), request)


@router.get("/{media_id}/file")
async def media_file(media_id: str, kind: str = "original", download: bool = False):
    """Redirect to a freshly signed read URL from the relay."""
    return _relay_redirect(media_id, kind, download)


@public_router.get("/file/{media_id}")
async def public_media_file(media_id: str, kind: str = "original", download: bool = False):
    """Header-free thumbnail/original redirect for the phone's <Image>.

    `/api/media/file/:id` is the exact path MARS builds from a MediaRef's
    `serviceUrl`, so a MediaRef minted here renders on both platforms.
    """
    return _relay_redirect(media_id, kind, download)
