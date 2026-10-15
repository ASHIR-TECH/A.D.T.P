from __future__ import annotations

import asyncio
import os
from unittest.mock import patch

import pytest
from httpx import ASGITransport, AsyncClient

from adtp_api.main import create_app

VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"


@pytest.fixture
def app(tmp_path):
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    os.environ["ADTP_AUDIT_DB"] = str(tmp_path / "audit.db")
    os.environ["ADTP_CLIENT_SOCKET"] = str(tmp_path / "client.sock")
    os.environ["ADTP_SERVER_SOCKET"] = str(tmp_path / "server.sock")
    os.environ["ADTP_AGENT_SOCKET"] = str(tmp_path / "agent.sock")
    app = create_app()
    yield app
    for var in (
        "ADTP_AUDIT_DB",
        "ADTP_CLIENT_SOCKET",
        "ADTP_SERVER_SOCKET",
        "ADTP_AGENT_SOCKET",
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
async def test_agent_status_requires_auth(client):
    resp = await client.get("/api/v1/agent/status")
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_agent_message_requires_auth(client):
    resp = await client.post(
        "/api/v1/agent/message", json={"message": "hello"}, headers={}
    )
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_agent_status_fallback_when_daemon_down(client):
    resp = await client.get("/api/v1/agent/status", headers=_auth())
    assert resp.status_code == 200
    data = resp.json()
    assert data["active"] is False
    assert data["provider"] is None
    assert data["model"] is None
    assert data["current_task_id"] is None


@pytest.mark.asyncio
async def test_message_not_found(client):
    resp = await client.get("/api/v1/agent/messages/does-not-exist", headers=_auth())
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_message_flow_transitions_running_to_completed(client):
    started = asyncio.Event()
    release = asyncio.Event()

    async def fake_call_socket(socket_path, cmd, params=None):
        assert cmd == "agent_message"
        assert params["message"] == "hello desktop"
        started.set()
        await release.wait()
        return {
            "text": "Sent file to peer and listed peers.",
            "provider": "anthropic",
            "model": "claude-sonnet",
            "tool_calls": [
                {
                    "name": "send_file",
                    "status": "completed",
                    "params": {"file_path": "/tmp/report.pdf", "peer_id": "peer-1"},
                    "result": "Sent /tmp/report.pdf to peer-1",
                },
                {
                    "name": "list_peers",
                    "status": "completed",
                    "params": {},
                    "result": "peer-1 (online)",
                },
            ],
        }

    with patch("adtp_mcp.bridge.base.call_socket", new=fake_call_socket):
        resp = await client.post(
            "/api/v1/agent/message",
            json={"message": "hello desktop"},
            headers=_auth(),
        )
        assert resp.status_code == 201
        message_id = resp.json()["id"]
        assert message_id

        await asyncio.wait_for(started.wait(), timeout=5)

        resp = await client.get(
            f"/api/v1/agent/messages/{message_id}", headers=_auth()
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "running"

        release.set()
        for _ in range(100):
            resp = await client.get(
                f"/api/v1/agent/messages/{message_id}", headers=_auth()
            )
            if resp.json()["status"] == "completed":
                break
            await asyncio.sleep(0.02)

    data = resp.json()
    assert data["status"] == "completed"
    assert data["text"] == "Sent file to peer and listed peers."
    assert data["provider"] == "anthropic"
    names = [tc.get("name") or tc.get("tool") for tc in data["tool_calls"]]
    assert "send_file" in names
    assert "list_peers" in names


@pytest.mark.asyncio
async def test_health_ready_stays_auth_exempt(client):
    resp = await client.get("/api/v1/health/ready")
    assert resp.status_code != 401


@pytest.mark.asyncio
async def test_prefers_desktop_agent_socket_when_present(app, client, tmp_path):
    """The embedded agent lives in the desktop app; when its socket exists,
    adtp-api must dispatch there instead of the (agent-less) daemon socket."""
    agent_socket = tmp_path / "agent.sock"
    agent_socket.touch()
    seen: dict[str, str] = {}

    async def fake_call_socket(socket_path, cmd, params=None):
        seen["socket"] = str(socket_path)
        return {"active": True, "provider": "ollama", "model": "llama3"}

    with patch("adtp_mcp.bridge.base.call_socket", new=fake_call_socket):
        resp = await client.get("/api/v1/agent/status", headers=_auth())

    assert resp.status_code == 200
    assert resp.json()["active"] is True
    assert resp.json()["provider"] == "ollama"
    assert seen["socket"] == str(agent_socket)