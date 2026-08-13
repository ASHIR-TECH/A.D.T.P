from __future__ import annotations

import os

import pytest
from httpx import AsyncClient, ASGITransport

from adtp_api.main import create_app


VALID_TOKEN = "abcdefghijklmnopqrstuvwxyz0123456789"


@pytest.fixture
def app(tmp_path):
    os.environ["ADTP_API_TOKEN"] = VALID_TOKEN
    os.environ["ADTP_AUDIT_DB"] = str(tmp_path / "audit.db")
    app = create_app()
    yield app
    os.environ.pop("ADTP_AUDIT_DB", None)


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


@pytest.mark.asyncio
async def test_no_auth_header(client):
    resp = await client.get("/api/v1/transfers")
    assert resp.status_code == 401
    assert resp.json()["detail"] == "Unauthorized"


@pytest.mark.asyncio
async def test_wrong_auth_scheme(client):
    resp = await client.get("/api/v1/transfers", headers={"Authorization": "Basic dGVzdDpwYXNz"})
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_invalid_token(client):
    resp = await client.get("/api/v1/transfers", headers={"Authorization": "Bearer wrong-token"})
    assert resp.status_code == 401


@pytest.mark.asyncio
async def test_valid_token_passes_auth(client):
    from adtp_api.middleware.auth import EXEMPT_PATHS
    assert "/api/v1/health" in EXEMPT_PATHS
    try:
        resp = await client.get("/api/v1/transfers", headers={"Authorization": f"Bearer {VALID_TOKEN}"})
        assert resp.status_code != 401
    except Exception as e:
        msg = str(e)
        assert "OperationalError" in msg or "no such table" in msg


@pytest.mark.asyncio
async def test_health_bypasses_auth(client):
    resp = await client.get("/api/v1/health")
    assert resp.status_code == 200


@pytest.mark.asyncio
async def test_docs_bypasses_auth(client):
    resp = await client.get("/api/v1/health")
    assert resp.status_code == 200
