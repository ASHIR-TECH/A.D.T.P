from __future__ import annotations

import hashlib
import os

import pytest
from httpx import ASGITransport, AsyncClient

from adtp_api.main import create_app

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"
DEVICE_ID = "phone-abc123"


@pytest.fixture
def app(tmp_path):
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    os.environ["ADTP_AUDIT_DB"] = str(tmp_path / "audit.db")
    os.environ["ADTP_CLIENT_SOCKET"] = str(tmp_path / "client.sock")
    os.environ["ADTP_SERVER_SOCKET"] = str(tmp_path / "server.sock")
    os.environ["ADTP_AGENT_SOCKET"] = str(tmp_path / "agent.sock")
    os.environ["ADTP_INTAKE_DIR"] = str(tmp_path / "intake")
    os.environ["ADTP_OUTBOX_DIR"] = str(tmp_path / "outbox")
    app = create_app()
    yield app
    for var in (
        "ADTP_AUDIT_DB",
        "ADTP_CLIENT_SOCKET",
        "ADTP_SERVER_SOCKET",
        "ADTP_AGENT_SOCKET",
        "ADTP_INTAKE_DIR",
        "ADTP_OUTBOX_DIR",
    ):
        os.environ.pop(var, None)


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


def _auth() -> dict[str, str]:
    return {"Authorization": f"Bearer {VALID_TOKEN}"}


def _registration(**overrides) -> dict:
    body = {
        "device_id": DEVICE_ID,
        "name": "Pixel 9",
        "platform": "android",
        "os_name": "Android",
        "os_version": "15",
        "app_version": "1.0.0",
        "model": "Pixel 9",
        "battery_percent": 76.0,
        "battery_charging": True,
        "capabilities": ["chat", "files"],
    }
    body.update(overrides)
    return body


@pytest.mark.asyncio
async def test_register_lists_and_unpairs_device(client):
    created = await client.post(
        "/api/v1/devices/register", json=_registration(), headers=_auth()
    )
    assert created.status_code == 201
    data = created.json()
    assert data["already_paired"] is False
    assert data["device"]["device_id"] == DEVICE_ID
    assert data["device"]["online"] is True
    assert data["device"]["battery_percent"] == 76.0

    listed = await client.get("/api/v1/devices", headers=_auth())
    assert listed.status_code == 200
    assert listed.json()["total"] == 1

    again = await client.post(
        "/api/v1/devices/register", json=_registration(), headers=_auth()
    )
    assert again.json()["already_paired"] is True

    removed = await client.delete(f"/api/v1/devices/{DEVICE_ID}", headers=_auth())
    assert removed.status_code == 200
    listed = await client.get("/api/v1/devices", headers=_auth())
    assert listed.json()["total"] == 0


@pytest.mark.asyncio
async def test_heartbeat_updates_telemetry(client):
    await client.post("/api/v1/devices/register", json=_registration(), headers=_auth())

    beat = await client.post(
        f"/api/v1/devices/{DEVICE_ID}/heartbeat",
        json={"battery_percent": 42.5, "battery_charging": False, "name": "Pixel 9 Pro"},
        headers=_auth(),
    )
    assert beat.status_code == 200
    device = beat.json()
    assert device["battery_percent"] == 42.5
    assert device["battery_charging"] is False
    assert device["name"] == "Pixel 9 Pro"


@pytest.mark.asyncio
async def test_desktop_file_is_queued_and_downloadable(client, tmp_path):
    await client.post("/api/v1/devices/register", json=_registration(), headers=_auth())
    payload = b"from desktop to phone"

    queued = await client.post(
        f"/api/v1/devices/{DEVICE_ID}/files",
        files={"file": ("brief.txt", payload, "text/plain")},
        headers=_auth(),
    )
    assert queued.status_code == 201
    file_id = queued.json()["file"]["id"]
    assert queued.json()["file"]["filename"] == "brief.txt"

    inbox = await client.get(f"/api/v1/devices/{DEVICE_ID}/files", headers=_auth())
    assert inbox.json()["total"] == 1

    download = await client.get(
        f"/api/v1/devices/{DEVICE_ID}/files/{file_id}", headers=_auth()
    )
    assert download.status_code == 200
    assert download.content == payload

    # A delivered file leaves the inbox.
    inbox = await client.get(f"/api/v1/devices/{DEVICE_ID}/files", headers=_auth())
    assert inbox.json()["total"] == 0
    assert (tmp_path / "outbox" / DEVICE_ID / "brief.txt").exists()


@pytest.mark.asyncio
async def test_phone_upload_appears_in_history(client, tmp_path):
    payload = b"phone attachment"
    upload = await client.post(
        "/api/v1/transfers/upload",
        files={"file": ("photo.jpg", payload, "image/jpeg")},
        data={"device_id": DEVICE_ID},
        headers=_auth(),
    )
    assert upload.status_code == 201
    body = upload.json()
    assert body["sha256"] == hashlib.sha256(payload).hexdigest()
    assert (tmp_path / "intake" / "photo.jpg").exists()

    history = await client.get("/api/v1/transfers", headers=_auth())
    assert history.status_code == 200
    transfers = history.json()["transfers"]
    assert transfers
    entry = transfers[0]
    assert entry["filename"] == "photo.jpg"
    assert entry["transfer_type"] == "push"
    assert entry["status"] == "delivered"

    # The generic download endpoint can now resolve the upload by transfer ID.
    download = await client.get(
        f"/api/v1/transfers/{body['id']}/download", headers=_auth()
    )
    assert download.status_code == 200
    assert download.content == payload

    chain = await client.post("/api/v1/audit/verify", headers=_auth())
    assert chain.status_code == 200
    assert chain.json()["valid"] is True


@pytest.mark.asyncio
async def test_device_id_is_validated_for_on_disk_paths(client):
    bad = await client.post(
        "/api/v1/devices/register",
        json=_registration(device_id="../escape"),
        headers=_auth(),
    )
    assert bad.status_code == 422
