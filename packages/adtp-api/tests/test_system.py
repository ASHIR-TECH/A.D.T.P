from __future__ import annotations

import os

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
async def test_system_info_requires_auth(client):
    resp = await client.get("/api/v1/system/info")
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_system_info_reports_host_identity(client, tmp_path):
    resp = await client.get("/api/v1/system/info", headers=_auth())
    assert resp.status_code == 200
    data = resp.json()

    assert data["hostname"]
    assert data["os"]
    assert data["arch"]
    assert data["cpu_count"] and data["cpu_count"] >= 1
    assert data["gateway_version"] == "0.1.0"
    assert data["intake_dir"] == str(tmp_path / "intake")
    # Agent block always present; inactive because no socket exists in tests.
    assert data["agent"]["active"] is False


@pytest.mark.asyncio
async def test_system_info_percentages_are_bounded(client):
    resp = await client.get("/api/v1/system/info", headers=_auth())
    data = resp.json()
    for key in ("cpu_percent", "ram_percent", "disk_percent"):
        value = data[key]
        if value is not None:
            assert 0.0 <= value <= 100.0
