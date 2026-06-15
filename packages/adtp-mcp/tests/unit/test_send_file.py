import pytest

from adtp_mcp.tools.send_file import SendFileInput, send_file


@pytest.mark.anyio
async def test_send_file_validates_and_returns_result(tmp_path, monkeypatch):
    file_path = tmp_path / "report.txt"
    file_path.write_text("hello")

    async def fake_enqueue_send(path, agent_name, priority):
        assert path == str(file_path.resolve())
        assert agent_name == "default"
        assert priority == "normal"
        return {"transfer_id": "abc", "status": "queued", "estimated_ms": 0}

    monkeypatch.setattr("adtp_mcp.bridge.client.enqueue_send", fake_enqueue_send)
    result = await send_file(SendFileInput(file_path=str(file_path)))

    assert result["transfer_id"] == "abc"
    assert result["filename"] == "report.txt"
    assert result["file_size"] == 5
