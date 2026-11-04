from __future__ import annotations

import os
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

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


def _auth_frame(
    token: str = VALID_TOKEN,
    desktop_id: str | None = None,
    device_id: str | None = None,
) -> dict:
    params: dict = {"token": token}
    if desktop_id:
        params["desktopId"] = desktop_id
    if device_id:
        params["deviceId"] = device_id
    return {
        "type": "command",
        "nodeId": "__auth__",
        "action": "authenticate",
        "params": params,
    }


def test_relay_rejects_bad_token(app):
    with TestClient(app) as client:
        with client.websocket_connect("/relay") as ws:
            ws.send_json(_auth_frame("wrong-token-wrong-token-wrong-token"))
            frame = ws.receive_json()
            assert frame["type"] == "auth_rejected"


def test_relay_acks_and_pushes_state_update(app):
    with TestClient(app) as client:
        with client.websocket_connect("/relay") as ws:
            ws.send_json(_auth_frame())
            assert ws.receive_json()["type"] == "auth_ack"

            frame = ws.receive_json()
            assert frame["type"] == "state_update"
            devices = frame["devices"]
            assert len(devices) == 1
            assert devices[0]["status"] == "online"
            assert devices[0]["name"]
            # No paired id was sent, so the gateway self-reports by hostname.
            assert devices[0]["id"] == devices[0]["name"]


def test_relay_reports_desktop_under_paired_id(app):
    """The phone sends the desktop id it paired with so one desktop = one card."""
    paired_id = "a" * 64
    with TestClient(app) as client:
        with client.websocket_connect("/relay") as ws:
            ws.send_json(_auth_frame(desktop_id=paired_id))
            assert ws.receive_json()["type"] == "auth_ack"

            frame = ws.receive_json()
            assert frame["type"] == "state_update"
            assert frame["devices"][0]["id"] == paired_id
            assert frame["devices"][0]["name"]


def test_relay_chat_message_returns_agent_reply(app):
    tool_calls = [
        {
            "id": "call-1",
            "name": "send_file_to_device",
            "status": "completed",
            "params": {"file_path": "/home/me/image.json"},
            "result": '{"device_file_id": "file-abc", "filename": "image.json"}',
        }
    ]

    async def fake_call_socket(socket_path, cmd, params=None):
        assert cmd == "agent_message"
        return {
            "text": "Your desktop is online.",
            "provider": "ollama",
            "tool_calls": tool_calls,
        }

    with patch("adtp_mcp.bridge.base.call_socket", new=fake_call_socket):
        with TestClient(app) as client:
            with client.websocket_connect("/relay") as ws:
                ws.send_json(_auth_frame())
                assert ws.receive_json()["type"] == "auth_ack"
                assert ws.receive_json()["type"] == "state_update"

                ws.send_json(
                    {"type": "chat_message", "sessionId": "default-session", "text": "status?"}
                )

                # Skip any interleaved state_update frames until the reply lands.
                for _ in range(20):
                    frame = ws.receive_json()
                    if frame["type"] == "chat_response":
                        break
                assert frame["type"] == "chat_response"
                assert frame["sessionId"] == "default-session"
                assert frame["text"] == "Your desktop is online."
                assert frame["providerLabel"] == "ollama"
                assert frame["timestamp"]
                # MARS renders AI-sent files from these tool calls.
                assert frame["toolCalls"] == tool_calls


DEVICE_ID = "phone-relay-1"


def _registration() -> dict:
    return {
        "device_id": DEVICE_ID,
        "name": "Relay Phone",
        "platform": "android",
        "os_name": "Android",
        "os_version": "16",
        "app_version": "1.0.0",
        "model": "Pixel 9",
        "capabilities": ["chat"],
    }


def _auth_header() -> dict[str, str]:
    return {"Authorization": f"Bearer {VALID_TOKEN}"}


def test_device_session_receives_desktop_ping_and_goes_offline(app):
    """The relay socket is the live presence channel the desktop can ping."""
    with TestClient(app) as client:
        assert (
            client.post(
                "/api/v1/devices/register", json=_registration(), headers=_auth_header()
            ).status_code
            == 201
        )

        with client.websocket_connect("/relay") as ws:
            ws.send_json(_auth_frame(device_id=DEVICE_ID))
            assert ws.receive_json()["type"] == "auth_ack"
            assert ws.receive_json()["type"] == "state_update"

            ping = client.post(
                f"/api/v1/devices/{DEVICE_ID}/ping", headers=_auth_header()
            )
            assert ping.status_code == 200
            assert ping.json()["online"] is True

            for _ in range(10):
                frame = ws.receive_json()
                if frame["type"] == "ping":
                    break
            assert frame["type"] == "ping"

        offline = client.post(
            f"/api/v1/devices/{DEVICE_ID}/ping", headers=_auth_header()
        )
        assert offline.json()["online"] is False


def test_relay_answers_phone_ping_with_pong(app):
    with TestClient(app) as client:
        with client.websocket_connect("/relay") as ws:
            ws.send_json(_auth_frame())
            assert ws.receive_json()["type"] == "auth_ack"
            assert ws.receive_json()["type"] == "state_update"

            ws.send_json({"type": "ping"})
            for _ in range(10):
                frame = ws.receive_json()
                if frame["type"] == "pong":
                    break
            assert frame["type"] == "pong"
            assert frame["timestamp"]
