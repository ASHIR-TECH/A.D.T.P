from __future__ import annotations

import hashlib
import os

import pytest
from httpx import ASGITransport, AsyncClient

from adtp_api.config import settings
from adtp_api.main import create_app

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"


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
    for var in (
        "ADTP_AUDIT_DB",
        "ADTP_CLIENT_SOCKET",
        "ADTP_SERVER_SOCKET",
        "ADTP_AGENT_SOCKET",
        "ADTP_INTAKE_DIR",
    ):
        os.environ.pop(var, None)


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


def _auth() -> dict[str, str]:
    return {"Authorization": f"Bearer {VALID_TOKEN}"}


@pytest.mark.asyncio
async def test_upload_requires_auth(client):
    resp = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("note.txt", b"hello", "text/plain")},
    )
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_upload_persists_file_to_intake(client, tmp_path):
    payload = b"mars upload payload"
    resp = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("briefing.txt", payload, "text/plain")},
        headers=_auth(),
    )
    assert resp.status_code == 201
    data = resp.json()

    assert data["filename"] == "briefing.txt"
    assert data["size"] == len(payload)
    assert data["sha256"] == hashlib.sha256(payload).hexdigest()
    assert data["content_type"] == "text/plain"

    written = tmp_path / "intake" / "briefing.txt"
    assert str(written) == data["path"]
    assert written.read_bytes() == payload


@pytest.mark.asyncio
async def test_upload_strips_client_path_traversal(client, tmp_path):
    resp = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("../../evil.txt", b"x", "text/plain")},
        headers=_auth(),
    )
    assert resp.status_code == 201
    data = resp.json()
    assert data["filename"] == "evil.txt"
    assert (tmp_path / "intake" / "evil.txt").exists()
    assert not (tmp_path.parent / "evil.txt").exists()


@pytest.mark.asyncio
async def test_upload_dedupes_existing_filename(client, tmp_path):
    files = {"file": ("report.txt", b"one", "text/plain")}
    first = await client.post("/api/v1/transfers/upload", files=files, headers=_auth())
    second = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("report.txt", b"two", "text/plain")},
        headers=_auth(),
    )
    assert first.json()["filename"] == "report.txt"
    assert second.json()["filename"] == "report-1.txt"


@pytest.mark.asyncio
async def test_upload_rejects_oversized_file(client, monkeypatch):
    monkeypatch.setattr(settings, "max_file_size", 4)
    resp = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("big.bin", b"12345", "application/octet-stream")},
        headers=_auth(),
    )
    assert resp.status_code == 413
