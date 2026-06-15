from __future__ import annotations

import asyncio
import shutil
import tempfile
from pathlib import Path

import pytest

from adtp_mcp.context.packager import EXTRACTOR_TIMEOUT, build_context
from adtp_mcp.context.extractors.csv import CsvExtractor

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"


@pytest.mark.anyio
async def test_context_flow_excel():
    """Full context flow: xlsx fixture → correct schema output"""
    result = await build_context(str(FIXTURES / "sample.xlsx"), session_id="integration-test")
    assert result["schema"] is not None
    assert "sheet_names" in result["schema"]
    assert len(result["schema"]["sheet_names"]) >= 2
    assert isinstance(result["summary"], str) and len(result["summary"]) > 0
    assert isinstance(result["agent_hint"], str) and len(result["agent_hint"]) > 0
    assert isinstance(result["extraction_ms"], int)
    assert result["extraction_ms"] >= 0


@pytest.mark.anyio
async def test_context_flow_pdf():
    """Full context flow: pdf fixture → correct schema output"""
    result = await build_context(str(FIXTURES / "sample.pdf"), session_id="integration-test")
    assert result["schema"] is not None
    assert result["schema"]["page_count"] >= 5
    assert isinstance(result["summary"], str) and len(result["summary"]) > 0


@pytest.mark.anyio
async def test_context_flow_csv():
    """Full context flow: csv fixture → correct schema output"""
    result = await build_context(str(FIXTURES / "sample.csv"), session_id="integration-test")
    assert result["schema"] is not None
    assert result["schema"]["header_row"] is not None
    assert len(result["schema"]["header_row"]) >= 5
    assert result["schema"]["row_count"] >= 100
    assert result["summary"]


@pytest.mark.anyio
async def test_context_flow_audio():
    """Full context flow: mp3 fixture → correct schema output"""
    result = await build_context(str(FIXTURES / "sample.mp3"), session_id="integration-test")
    assert result["schema"] is not None
    assert "duration_seconds" in result["schema"]
    if result["schema"]:
        assert isinstance(result["extraction_ms"], int)


@pytest.mark.anyio
async def test_context_flow_json():
    """Full context flow: json fixture → correct schema output"""
    result = await build_context(str(FIXTURES / "sample.json"), session_id="integration-test")
    assert result["schema"] is not None
    assert result["schema"]["top_level_type"] == "array"
    assert result["schema"]["array_length"] == 3


@pytest.mark.anyio
async def test_context_flow_generic():
    """Full context flow: bin fixture → minimal schema"""
    result = await build_context(str(FIXTURES / "sample.bin"), session_id="integration-test")
    assert result["schema"] is not None
    assert result["schema"]["mime_type"] == "application/octet-stream"
    assert result["summary"] is not None
    assert result["agent_hint"] == "Binary file — process by filename and size only"


@pytest.mark.anyio
async def test_timeout_handling():
    """Extractor that sleeps past the timeout should not block the tool call."""
    _original_extract = CsvExtractor.extract

    def slow_extract(file_path):
        import time
        time.sleep(EXTRACTOR_TIMEOUT + 2)
        return _original_extract(file_path)

    CsvExtractor.extract = staticmethod(slow_extract)
    try:
        result = await build_context(str(FIXTURES / "sample.csv"), session_id="timeout-test")
        assert result["schema"] is None
        assert result["summary"] is None
    finally:
        CsvExtractor.extract = staticmethod(_original_extract)


@pytest.mark.anyio
async def test_corrupt_file():
    """Corrupt file should not block transfer — empty context returned."""
    tmp = tempfile.NamedTemporaryFile(suffix=".xlsx", delete=False)
    tmp.write(b"this is not a valid xlsx file at all")
    tmp.close()
    try:
        result = await build_context(tmp.name, session_id="corrupt-test")
        assert result["schema"] is None or result["schema"] != {}
    finally:
        Path(tmp.name).unlink(missing_ok=True)


@pytest.mark.anyio
async def test_missing_file():
    """Missing file should return empty context."""
    result = await build_context("/tmp/does-not-exist-12345.xyz", session_id="missing-test")
    assert result["schema"] is None


@pytest.mark.anyio
async def test_context_includes_transfer_history():
    """History list should be included in result."""
    result = await build_context(str(FIXTURES / "sample.xlsx"), session_id="history-test")
    assert "transfer_history" in result
    assert isinstance(result["transfer_history"], list)


@pytest.mark.anyio
async def test_all_fixtures_produce_context():
    """Every test fixture should produce a non-None schema (even if minimal)."""
    for fname in FIXTURES.iterdir():
        if fname.suffix in (".py", ".mp3"):  # generator script, mp3 ok
            continue
        result = await build_context(str(fname), session_id="all-fixtures-test")
        assert result["schema"] is not None, f"Fixture {fname.name} produced no schema"
        assert isinstance(result["extraction_ms"], int)
