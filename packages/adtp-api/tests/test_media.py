"""Tests for the shared media contract (no network calls)."""
from __future__ import annotations

import io
import json

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
