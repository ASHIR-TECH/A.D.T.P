import pytest
from pydantic import ValidationError

from adtp_mcp.tools.list_transfers import ListTransfersInput, list_transfers


@pytest.mark.anyio
async def test_list_transfers_calls_bridge(monkeypatch):
    async def fake_get_transfers(status, limit):
        assert status == "all"
        assert limit == 10
        return {"transfers": [{"id": "1"}]}

    monkeypatch.setattr("adtp_mcp.bridge.client.get_transfers", fake_get_transfers)
    assert await list_transfers(ListTransfersInput()) == [{"id": "1"}]


def test_list_transfers_limit_bounds():
    ListTransfersInput(limit=1)
    ListTransfersInput(limit=100)
    with pytest.raises(ValidationError):
        ListTransfersInput(limit=0)
    with pytest.raises(ValidationError):
        ListTransfersInput(limit=101)
