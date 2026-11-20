"""Media travels over the same chat/device frames in both directions."""
from __future__ import annotations

import os
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

from adtp_api.main import create_app

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"

MEDIA = {
    "id": "uuid-wire-1",
    "serviceUrl": "https://relay.example",
    "name": "render.png",
    "mimeType": "image/png",
    "size": 9000,
    "compressedSize": 3000,
    "compressed": True,
    "hasThumbnail": True,
    "createdAt": "2026-11-20T00:00:00Z",
}


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


def _auth_frame(device_id="phone-wire-1"):
    return {
        "type": "command",
        "nodeId": "__auth__",
        "action": "authenticate",
        "params": {"token": VALID_TOKEN, "deviceId": device_id},
    }


def _chat_response(ws):
    for _ in range(30):
        frame = ws.receive_json()
        if frame["type"] == "chat_response":
            return frame
    raise AssertionError("no chat_response received")


def test_mars_media_is_echoed_to_both_sides(app):
    """MARS -> ADTP: phone media in chat_message comes back in chat_response."""
    async def fake_call_socket(socket_path, cmd, params=None):
        return {"text": "Got it.", "provider": "adtp-ai", "tool_calls": []}

    with patch("adtp_mcp.bridge.base.call_socket", new=fake_call_socket):
        with TestClient(app) as client:
            with client.websocket_connect("/relay") as ws:
                ws.send_json(_auth_frame())
                assert ws.receive_json()["type"] == "auth_ack"
                ws.receive_json()  # state_update
                ws.send_json({
                    "type": "chat_message",
                    "sessionId": "default-session",
                    "text": "here is a photo",
                    "media": MEDIA,
                })
                frame = _chat_response(ws)
                assert frame["media"]["id"] == MEDIA["id"]
                assert frame["media"]["serviceUrl"] == MEDIA["serviceUrl"]


def test_adtp_agent_tool_media_reaches_the_phone(app):
    """ADTP -> MARS: a MediaRef inside a tool result becomes chat_response.media."""
    tool_calls = [{
        "id": "call-media",
        "name": "send_media_to_device",
        "status": "completed",
        "params": {"file_path": "/home/me/render.png"},
        "result": '{"media": ' + __import__("json").dumps(MEDIA) + '}',
    }]

    async def fake_call_socket(socket_path, cmd, params=None):
        return {"text": "Sent the render.", "provider": "adtp-ai", "tool_calls": tool_calls}

    with patch("adtp_mcp.bridge.base.call_socket", new=fake_call_socket):
        with TestClient(app) as client:
            with client.websocket_connect("/relay") as ws:
                ws.send_json(_auth_frame())
                assert ws.receive_json()["type"] == "auth_ack"
                ws.receive_json()
                ws.send_json({"type": "chat_message", "sessionId": "default-session", "text": "render it"})
                frame = _chat_response(ws)
                assert frame["media"]["name"] == "render.png"
                assert frame["media"]["hasThumbnail"] is True


def test_device_message_keeps_media_durably(app):
    headers = {"Authorization": f"Bearer {VALID_TOKEN}"}
    with TestClient(app) as client:
        device_id = "phone-wire-1"
        client.post("/api/v1/devices/register", headers=headers, json={"device_id": device_id})
        response = client.post(
            f"/api/v1/devices/{device_id}/messages",
            headers=headers,
            json={"text": "AI sent a render", "media": MEDIA},
        )
        assert response.status_code == 201
        messages = client.get(
            f"/api/v1/devices/{device_id}/messages", headers=headers
        ).json()["messages"]
        assert messages[0]["media"]["id"] == MEDIA["id"]
