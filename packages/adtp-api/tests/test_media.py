"""Tests for the shared media contract (no network calls)."""
from __future__ import annotations

import io
import json

from PIL import Image

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

    compressed, thumb, width, height = media_service.compress_image(original, "image/png")
    assert len(compressed) < len(original)
    assert thumb and len(thumb) > 0
    assert (width, height) == (1600, 1067)
    with Image.open(io.BytesIO(thumb)) as thumb_image:
        assert max(thumb_image.size) <= media_service.THUMBNAIL_EDGE


def test_prepare_passes_non_images_through():
    data = b"%PDF-1.4 ..."
    out, thumb, width, height, compressed = media_service.prepare(data, "application/pdf", "a.pdf")
    assert out == data
    assert thumb is None and width is None and compressed is False
