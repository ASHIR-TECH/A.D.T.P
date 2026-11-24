"""Gateway media contract: the phone uploads and reads through the desktop.

The phone may not reach the relay directly, so `/api/v1/media/upload` rewrites
`serviceUrl` to the gateway and `/api/media/file/:id` is header-free so an
<Image> can load it. These tests pin both behaviours without touching the net.
"""
from __future__ import annotations

import os
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

from adtp_api.main import create_app
from adtp_api.models.media import MediaRef

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"
AUTH = {"Authorization": f"Bearer {VALID_TOKEN}"}

MEDIA = MediaRef(
    id="uuid-gateway-1",
    serviceUrl="https://relay.example",
    name="shot.png",
    mimeType="image/png",
    size=9000,
    compressedSize=3000,
    compressed=True,
    hasThumbnail=True,
    createdAt="2026-11-20T00:00:00Z",
)


@pytest.fixture
def app(tmp_path):
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    os.environ["ADTP_AUDIT_DB"] = str(tmp_path / "audit.db")
    os.environ["ADTP_CLIENT_SOCKET"] = str(tmp_path / "client.sock")
    os.environ["ADTP_SERVER_SOCKET"] = str(tmp_path / "server.sock")
    os.environ["ADTP_AGENT_SOCKET"] = str(tmp_path / "agent.sock")
    os.environ["ADTP_INTAKE_DIR"] = str(tmp_path / "intake")
    app = create_app()
    yield app
    for var in ("ADTP_AUDIT_DB", "ADTP_CLIENT_SOCKET", "ADTP_SERVER_SOCKET",
                "ADTP_AGENT_SOCKET", "ADTP_INTAKE_DIR"):
        os.environ.pop(var, None)


def test_upload_rewrites_service_url_to_the_gateway(app):
    async def fake_store_upload(file, **kwargs):
        return MEDIA.model_copy()

    with patch("adtp_api.services.media_service.store_upload", new=fake_store_upload):
        with TestClient(app) as client:
            response = client.post(
                "/api/v1/media/upload",
                headers=AUTH,
                files={"file": ("shot.png", b"not-a-real-png", "image/png")},
                data={"sender": "mars-user"},
            )
    assert response.status_code == 201
    # The phone must read through the host it already reached, not the relay.
    assert response.json()["serviceUrl"] == "http://testserver"


def test_public_media_file_is_header_free(app):
    """No bearer token: the random id is the capability (<Image> can't send headers)."""
    with TestClient(app) as client:
        response = client.get(
            "/api/media/file/uuid-gateway-1?kind=thumbnail", follow_redirects=False
        )
    assert response.status_code != 401
    # A configured relay answers with a redirect the image loader can follow.
    assert response.status_code in (302, 503)
    if response.status_code == 302:
        assert "/api/media/file/uuid-gateway-1" in response.headers["location"]


def test_private_media_routes_still_require_auth(app):
    with TestClient(app) as client:
        assert client.get("/api/v1/media/uuid-gateway-1").status_code == 401
        assert client.post("/api/v1/media/upload", files={"file": ("x.png", b"x", "image/png")}).status_code == 401
