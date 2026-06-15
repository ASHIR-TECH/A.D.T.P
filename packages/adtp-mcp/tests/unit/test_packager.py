from __future__ import annotations

from pathlib import Path

import pytest

from adtp_mcp.context.packager import build_context

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"


@pytest.mark.anyio
async def test_packager_excel():
    result = await build_context(str(FIXTURES / "sample.xlsx"), session_id="test-session")
    assert isinstance(result, dict)
    assert result["schema"] is not None
    assert result["schema"].get("sheet_names") is not None
    assert isinstance(result["summary"], str)
    assert isinstance(result["agent_hint"], str)
    assert isinstance(result["transfer_history"], list)
    assert isinstance(result["extraction_ms"], int)


@pytest.mark.anyio
async def test_packager_pdf():
    result = await build_context(str(FIXTURES / "sample.pdf"), session_id="test-session")
    assert result["schema"] is not None
    assert result["schema"]["page_count"] >= 5


@pytest.mark.anyio
async def test_packager_csv():
    result = await build_context(str(FIXTURES / "sample.csv"), session_id="test-session")
    assert result["schema"] is not None
    assert result["schema"]["header_row"] is not None


@pytest.mark.anyio
async def test_packager_json():
    result = await build_context(str(FIXTURES / "sample.json"), session_id="test-session")
    assert result["schema"] is not None
    assert result["schema"]["top_level_type"] == "array"


@pytest.mark.anyio
async def test_packager_generic():
    result = await build_context(str(FIXTURES / "sample.bin"), session_id="test-session")
    assert result["schema"] is not None
    assert result["schema"]["mime_type"] == "application/octet-stream"


@pytest.mark.anyio
async def test_packager_nonexistent_file():
    result = await build_context("/nonexistent/file.pdf", session_id="test-session")
    assert result["schema"] is None


@pytest.mark.anyio
async def test_packager_no_session():
    result = await build_context(str(FIXTURES / "sample.csv"))
    assert result["schema"] is not None
    assert result["transfer_history"] == []
