import pytest

from adtp_mcp.resources import pending_files, session_status, transfer_log


@pytest.mark.anyio
async def test_transfer_log_resource(monkeypatch):
    async def fake_get_transfers(status, limit):
        return {"transfers": [], "status": status, "limit": limit}

    monkeypatch.setattr("adtp_mcp.bridge.client.get_transfers", fake_get_transfers)
    assert await transfer_log.read() == {"transfers": [], "status": "all", "limit": 100}


@pytest.mark.anyio
async def test_session_status_resource(monkeypatch):
    async def fake_session_status():
        return {"uptime_ms": 1}

    monkeypatch.setattr("adtp_mcp.bridge.client.session_status", fake_session_status)
    assert await session_status.read() == {"uptime_ms": 1}


@pytest.mark.anyio
async def test_pending_files_resource(monkeypatch):
    async def fake_pending_files():
        return {"pending": []}

    monkeypatch.setattr("adtp_mcp.bridge.client.pending_files", fake_pending_files)
    assert await pending_files.read() == {"pending": []}
