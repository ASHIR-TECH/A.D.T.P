"""Tests for the shared media contract (no network calls)."""
from __future__ import annotations

import io
import json
from types import SimpleNamespace

import httpx
import pytest
from PIL import Image

from adtp_api.config import settings
from adtp_api.models.media import MediaRef, media_from_tool_calls, normalize_media_ref
from adtp_api.services import media_service


def _ref(**overrides: object) -> dict:
    ref = {
        "id": "uuid-1",
        "serviceUrl": "https://relay.example",
        "name": "photo.png",
        "mimeType": "image/png",
        "size": 9000,
        "compressedSize": 3000,
        "compressed": True,
        "hasThumbnail": True,
        "createdAt": "2026-11-20T00:00:00Z",
    }
    ref.update(overrides)
    return ref


def test_normalize_media_ref_accepts_valid_and_rejects_unsafe():
    assert normalize_media_ref(_ref()) is not None
    assert normalize_media_ref(None) is None
    assert normalize_media_ref({"id": "x"}) is None
    assert normalize_media_ref(_ref(serviceUrl="javascript:alert(1)")) is None
    assert normalize_media_ref(_ref(serviceUrl="https://user:pass@relay.example")) is None


def test_v1_reference_is_upgraded_to_v2():
    ref = normalize_media_ref(_ref())
    assert ref is not None
    assert ref.schemaVersion == 1
    assert ref.storedSize == 3000
    assert ref.compressedSize == 3000
    assert ref.status == "ready"
    assert ref.as_wire()["storedSize"] == 3000


def test_legacy_relay_statuses_map_onto_the_state_machine():
    assert normalize_media_ref(_ref(status="pending")).status == "created"
    assert normalize_media_ref(_ref(status="complete")).status == "ready"


def test_expiry_is_reported_from_the_reference():
    ref = normalize_media_ref(_ref(expiresAt="2020-01-01T00:00:00Z"))
    assert ref is not None and ref.expired is True
    future = normalize_media_ref(_ref(expiresAt="2999-01-01T00:00:00Z"))
    assert future is not None and future.expired is False
    assert normalize_media_ref(_ref(status="deleted")).expired is True


def test_media_from_tool_calls_extracts_blob_reference():
    calls = [
        {
            "name": "send_media_to_device",
            "status": "completed",
            "result": json.dumps({"media": _ref(name="render.png")}),
        },
        {"name": "list_devices", "status": "completed", "result": "..."},
    ]
    found = media_from_tool_calls(calls)
    assert found and found.name == "render.png"
    assert media_from_tool_calls([{**calls[0], "status": "failed"}]) is None


def test_compress_image_reduces_size_and_makes_thumbnail():
    image = Image.new("RGB", (2400, 1600), (120, 40, 200))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    original = buffer.getvalue()

    compressed, thumb, width, height, mime = media_service.compress_image(original, "image/png")
    assert len(compressed) < len(original)
    assert mime == "image/jpeg"
    assert thumb and len(thumb) > 0
    assert (width, height) == (1600, 1067)
    with Image.open(io.BytesIO(thumb)) as thumb_image:
        assert max(thumb_image.size) <= media_service.THUMBNAIL_EDGE


def test_prepare_non_image_preserves_bytes_and_name():
    data = b"%PDF-1.4 ..."
    out = media_service.prepare(data, "application/pdf", "../../a.pdf")
    assert out.data == data
    assert out.thumbnail is None and out.width is None and out.compressed is False
    assert out.mime_type == "application/pdf"
    assert out.name == "a.pdf"


def test_prepare_jpeg_conversion_uses_matching_mime_and_filename():
    image = Image.new("RGB", (40, 30), (10, 20, 30))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    out = media_service.prepare(buffer.getvalue(), "image/png", "capture.png")
    assert out.compressed is True
    assert out.mime_type == "image/jpeg"
    assert out.name == "capture.jpg"
    with Image.open(io.BytesIO(out.data)) as stored:
        assert stored.format == "JPEG"


def test_prepare_preserves_png_transparency():
    image = Image.new("RGBA", (40, 30), (10, 20, 30, 0))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    out = media_service.prepare(buffer.getvalue(), "image/png", "logo.png")
    assert out.mime_type == "image/png"
    assert out.name == "logo.png"
    with Image.open(io.BytesIO(out.data)) as stored:
        assert "A" in stored.getbands()


def test_prepare_preserves_animated_gif():
    frames = [Image.new("RGB", (20, 20), color) for color in ("red", "blue")]
    buffer = io.BytesIO()
    frames[0].save(buffer, format="GIF", save_all=True, append_images=frames[1:])
    data = buffer.getvalue()
    out = media_service.prepare(data, "image/gif", "loop.gif")
    assert out.data == data
    assert out.compressed is False
    assert out.mime_type == "image/gif"


class _FakeUpload:
    """Minimal UploadFile shape used by the streaming spool test."""

    def __init__(self, chunks: list[bytes], filename: str = "x.bin", content_type: str = "application/octet-stream"):
        self._chunks = list(chunks)
        self.filename = filename
        self.content_type = content_type

    async def read(self, size: int) -> bytes:
        return self._chunks.pop(0) if self._chunks else b""


@pytest.mark.asyncio
async def test_spool_upload_enforces_limit_while_streaming(monkeypatch):
    monkeypatch.setattr(settings, "max_media_size", 8)
    with pytest.raises(Exception) as caught:
        await media_service._spool_upload(_FakeUpload([b"12345", b"6789"]))
    assert getattr(caught.value, "status_code", None) == 413


@pytest.mark.asyncio
async def test_spool_upload_streams_hash_and_size(monkeypatch):
    monkeypatch.setattr(settings, "max_media_size", 1024)
    path, size, sha = await media_service._spool_upload(_FakeUpload([b"abc", b"def"]))
    try:
        assert size == 6
        assert sha == media_service.digest(b"abcdef")
    finally:
        await media_service._remove(path)


class _FakeHttpError(httpx.HTTPError):
    def __init__(self, status: int):
        super().__init__(f"status {status}")
        self.response = SimpleNamespace(status_code=status)


class _FakeResponse:
    def __init__(self, data: dict, status: int = 200):
        self._data = data
        self._status = status

    def raise_for_status(self) -> None:
        if self._status >= 400:
            raise _FakeHttpError(self._status)

    def json(self) -> dict:
        return self._data


@pytest.mark.asyncio
async def test_store_emits_v2_reference_using_the_mars_relay_shape(monkeypatch):
    """End-to-end ADTP sender check against a v2-shaped relay response."""
    captured: dict = {}

    def mars_ref(sign: dict) -> dict:
        stored = sign["storedSize"]
        return {
            "schemaVersion": 2,
            "id": "media-1",
            "serviceUrl": "https://relay.example",
            "name": sign["name"],
            "mimeType": sign["mimeType"],
            "size": sign["size"],
            "compressedSize": stored,
            "storedSize": stored,
            "sha256": sign["sha256"],
            "sender": sign["sender"],
            "recipient": sign.get("recipient"),
            "conversationId": sign.get("conversationId"),
            "sessionId": sign.get("sessionId"),
            "compressed": sign["compressed"],
            "hasThumbnail": True,
            "createdAt": "2026-11-26T10:00:00Z",
            "expiresAt": "2026-11-27T10:00:00Z",
            "status": "ready",
        }

    class FakeClient:
        def __init__(self, *args, **kwargs):
            pass

        async def __aenter__(self):
            return self

        async def __aexit__(self, *args):
            return False

        async def post(self, url, headers=None, json=None):
            if url.endswith("/sign"):
                captured["sign"] = json
                return _FakeResponse({
                    "id": "media-1",
                    "original": {"url": "https://blob.example/original", "headers": {}},
                    "thumbnail": {"url": "https://blob.example/thumb", "headers": {}},
                })
            if url.endswith("/complete"):
                captured["complete"] = json
                return _FakeResponse({"media": mars_ref(captured["sign"])})
            raise AssertionError(f"unexpected POST {url}")

        async def put(self, url, headers=None, content=None):
            body = content if isinstance(content, bytes) else b"".join([c async for c in content])
            captured.setdefault("puts", []).append((url, body))
            return _FakeResponse({})

        async def get(self, url, headers=None):
            captured.setdefault("gets", []).append(url)
            return _FakeResponse({"media": mars_ref(captured["sign"])})

    monkeypatch.setattr(settings, "relay_url", "https://relay.example")
    monkeypatch.setattr(settings, "media_token", "server-token")
    monkeypatch.setattr(media_service.httpx, "AsyncClient", FakeClient)
    image = Image.new("RGB", (40, 30), (11, 22, 33))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")

    ref = await media_service.store(
        buffer.getvalue(),
        name="capture.png",
        mime_type="image/png",
        sender="adtp-desktop",
        conversation_id="default-session",
        recipient="phone-1",
    )

    original = next(body for url, body in captured["puts"] if url.endswith("original"))
    assert captured["sign"]["mimeType"] == "image/jpeg"
    assert captured["sign"]["name"] == "capture.jpg"
    assert captured["sign"]["storedSize"] == len(original)
    assert captured["sign"]["recipient"] == "phone-1"
    assert captured["sign"]["conversationId"] == "default-session"
    assert captured["complete"]["sha256"] == media_service.digest(original)
    assert captured["gets"]
    assert ref.schemaVersion == 2
    assert ref.expiresAt == "2026-11-27T10:00:00Z"
    assert ref.status == "ready"
    assert ref.storedSize == ref.compressedSize == len(original)
    assert ref.sha256 == media_service.digest(original)
