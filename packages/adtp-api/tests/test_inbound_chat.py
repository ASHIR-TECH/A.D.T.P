"""Phone → desktop files must become chat artifacts with a download path."""
from __future__ import annotations

import os

import pytest
from httpx import ASGITransport, AsyncClient

from adtp_api.main import create_app

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"
DEVICE_ID = "phone-inbound-chat"


@pytest.fixture
async def client(tmp_path):
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    os.environ["ADTP_AUDIT_DB"] = str(tmp_path / "audit.db")
    os.environ["ADTP_CLIENT_SOCKET"] = str(tmp_path / "client.sock")
    os.environ["ADTP_SERVER_SOCKET"] = str(tmp_path / "server.sock")
    os.environ["ADTP_AGENT_SOCKET"] = str(tmp_path / "agent.sock")
    os.environ["ADTP_INTAKE_DIR"] = str(tmp_path / "intake")
    os.environ["ADTP_OUTBOX_DIR"] = str(tmp_path / "outbox")
    app = create_app()
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac
    for var in (
        "ADTP_AUDIT_DB",
        "ADTP_CLIENT_SOCKET",
        "ADTP_SERVER_SOCKET",
        "ADTP_AGENT_SOCKET",
        "ADTP_INTAKE_DIR",
        "ADTP_OUTBOX_DIR",
    ):
        os.environ.pop(var, None)


def _auth() -> dict[str, str]:
    return {"Authorization": f"Bearer {VALID_TOKEN}"}


@pytest.mark.asyncio
async def test_phone_upload_appears_in_chat_and_is_downloadable(client):
    payload = b"file from mars"
    registered = await client.post(
        "/api/v1/devices/register",
        headers=_auth(),
        json={"device_id": DEVICE_ID, "name": "MARS", "platform": "android"},
    )
    assert registered.status_code == 201

    upload = await client.post(
        "/api/v1/transfers/upload",
        headers=_auth(),
        files={"file": ("from-phone.txt", payload, "text/plain")},
        data={"device_id": DEVICE_ID},
    )
    assert upload.status_code == 201
    transfer_id = upload.json()["id"]

    # The same durable inbox the two chat surfaces read must now contain it.
    messages = await client.get(
        f"/api/v1/devices/{DEVICE_ID}/messages?include_delivered=true", headers=_auth()
    )
    assert messages.status_code == 200
    inbound = [m for m in messages.json()["messages"] if m.get("direction") == "inbound"]
    assert len(inbound) == 1
    assert inbound[0]["text"] == "📎 from-phone.txt"
    media = inbound[0]["media"]
    assert media["id"] == transfer_id
    assert media["schemaVersion"] == 2
    assert media["storedSize"] == len(payload)

    # The MediaRef download URL must return the real bytes, header-free.
    downloaded = await client.get(
        f"/api/media/file/{transfer_id}?kind=original&download=1",
        follow_redirects=False,
    )
    assert downloaded.status_code == 200
    assert downloaded.content == payload

    # History keeps timestamp + actor/IP for the same event.
    audit = await client.get("/api/v1/audit/log", headers=_auth())
    assert audit.status_code == 200
    entry = audit.json()["entries"][0]
    assert entry["timestamp"]
    assert entry["actor"] == DEVICE_ID
    assert isinstance(entry["ip"], str)
