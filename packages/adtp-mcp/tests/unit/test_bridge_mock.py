import json

import pytest

from adtp_mcp.bridge.base import BridgeError, call_socket


class FakeReader:
    def __init__(self, line: bytes):
        self.line = line

    async def readline(self):
        return self.line


class FakeWriter:
    def __init__(self):
        self.writes = []
        self.closed = False

    def write(self, data: bytes):
        self.writes.append(data)

    async def wait_closed(self):
        return None


@pytest.mark.anyio
async def test_bridge_sends_newline_json_and_parses_response(monkeypatch, tmp_path):
    writer = FakeWriter()

    async def fake_open_unix_connection(path):
        assert path == str(tmp_path / "adtp.sock")
        return FakeReader(b'{"ok":true,"data":{"status":"queued"}}\n'), writer

    monkeypatch.setattr("asyncio.open_unix_connection", fake_open_unix_connection)
    response = await call_socket(tmp_path / "adtp.sock", "send", {"file_path": "/tmp/a.txt"})

    assert json.loads(writer.writes[0].decode()) == {"cmd": "send", "params": {"file_path": "/tmp/a.txt"}}
    assert writer.writes[0].endswith(b"\n")
    assert response == {"status": "queued"}
    assert writer.closed


@pytest.mark.anyio
async def test_bridge_raises_on_daemon_error(monkeypatch, tmp_path):
    async def fake_open_unix_connection(path):
        return FakeReader(b'{"ok":false,"error":"boom"}\n'), FakeWriter()

    monkeypatch.setattr("asyncio.open_unix_connection", fake_open_unix_connection)
    with pytest.raises(BridgeError):
        await call_socket(tmp_path / "adtp.sock", "send", {})
