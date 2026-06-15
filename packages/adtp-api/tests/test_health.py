from __future__ import annotations

import os
from unittest.mock import patch, MagicMock

import pytest
from httpx import AsyncClient, ASGITransport

from adtp_api.main import create_app


VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"


@pytest.fixture
def app():
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    app = create_app()
    yield app


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


@pytest.mark.asyncio
async def test_health_ok(client):
    resp = await client.get("/api/v1/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["version"] == "0.1.0"
    assert "uptime_seconds" in data


@pytest.mark.asyncio
async def test_health_ready_daemon_offline(client):
    resp = await client.get("/api/v1/health/ready")
    assert resp.status_code == 503
    data = resp.json()
    assert data["ready"] is False
    assert data["daemon_connected"] is False


@pytest.mark.asyncio
async def test_health_queue(client):
    resp = await client.get("/api/v1/health/queue")
    assert resp.status_code == 200
    data = resp.json()
    assert data["pending"] == -1
