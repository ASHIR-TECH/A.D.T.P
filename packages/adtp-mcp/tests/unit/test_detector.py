from __future__ import annotations

from pathlib import Path

import pytest

from adtp_mcp.context.detector import AdtpContextError, detect

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"


def test_detect_xlsx():
    mime = detect(str(FIXTURES / "sample.xlsx"))
    assert mime == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"


def test_detect_pdf():
    mime = detect(str(FIXTURES / "sample.pdf"))
    assert mime == "application/pdf"


def test_detect_docx():
    mime = detect(str(FIXTURES / "sample.docx"))
    assert mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"


def test_detect_csv():
    mime = detect(str(FIXTURES / "sample.csv"))
    assert mime == "text/csv"


def test_detect_mp3():
    mime = detect(str(FIXTURES / "sample.mp3"))
    assert "mpeg" in mime


def test_detect_mp4():
    mime = detect(str(FIXTURES / "sample.mp4"))
    assert mime == "video/mp4"


def test_detect_jpg():
    mime = detect(str(FIXTURES / "sample.jpg"))
    assert mime == "image/jpeg"


def test_detect_json():
    mime = detect(str(FIXTURES / "sample.json"))
    assert mime == "application/json"


def test_detect_bin():
    mime = detect(str(FIXTURES / "sample.bin"))
    assert mime == "application/octet-stream"


def test_detect_renamed_extension():
    """Detection by content, not extension — rename .xlsx to .txt"""
    import shutil
    import tempfile

    src = FIXTURES / "sample.xlsx"
    with tempfile.NamedTemporaryFile(suffix=".txt", delete=False) as tmp:
        tmp_path = tmp.name
        shutil.copy2(str(src), tmp_path)
    try:
        mime = detect(tmp_path)
        assert mime == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    finally:
        Path(tmp_path).unlink(missing_ok=True)


def test_detect_file_not_found():
    with pytest.raises(AdtpContextError):
        detect("/nonexistent/file.xyz")


def test_detect_empty_file():
    import tempfile

    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tmp:
        tmp_path = tmp.name
    try:
        mime = detect(tmp_path)
        assert mime in ("application/octet-stream", "inode/x-empty")
    finally:
        Path(tmp_path).unlink(missing_ok=True)
