from __future__ import annotations

from pathlib import Path

from adtp_mcp.context.extractors.csv import CsvExtractor
from adtp_mcp.context.extractors.excel import ExcelExtractor
from adtp_mcp.context.extractors.generic import GenericExtractor
from adtp_mcp.context.extractors.image import ImageExtractor
from adtp_mcp.context.extractors.json import JsonExtractor
from adtp_mcp.context.extractors.pdf import PdfExtractor
from adtp_mcp.context.extractors.video import VideoExtractor
from adtp_mcp.context.extractors.word import WordExtractor

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"


def _check_base(result):
    assert result.schema is not None
    assert isinstance(result.summary, str)
    assert isinstance(result.agent_hint, str)
    assert isinstance(result.extraction_ms, int)
    assert result.extraction_ms >= 0


class TestExcelExtractor:
    def test_extract_xlsx(self):
        result = ExcelExtractor.extract(str(FIXTURES / "sample.xlsx"))
        _check_base(result)
        assert len(result.schema["sheet_names"]) >= 2
        assert len(result.schema["headers"]) > 0
        assert result.schema["row_count"] > 0
        assert len(result.agent_hint) > 0

    def test_extract_failure(self):
        result = ExcelExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestPdfExtractor:
    def test_extract_pdf(self):
        result = PdfExtractor.extract(str(FIXTURES / "sample.pdf"))
        _check_base(result)
        assert result.schema["page_count"] >= 5
        assert result.schema["title"] or result.schema["author"]

    def test_extract_failure(self):
        result = PdfExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestWordExtractor:
    def test_extract_docx(self):
        result = WordExtractor.extract(str(FIXTURES / "sample.docx"))
        _check_base(result)
        assert result.schema["word_count"] > 0
        assert result.schema["heading_count"] > 0
        assert result.schema["heading_count"] >= 3
        assert result.schema["table_count"] >= 1

    def test_extract_failure(self):
        result = WordExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestCsvExtractor:
    def test_extract_csv(self):
        result = CsvExtractor.extract(str(FIXTURES / "sample.csv"))
        _check_base(result)
        assert result.schema["header_row"] is not None
        assert len(result.schema["header_row"]) > 0
        assert result.schema["row_count"] > 0
        assert result.schema["delimiter"] == ","

    def test_extract_failure(self):
        result = CsvExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestImageExtractor:
    def test_extract_jpg(self):
        result = ImageExtractor.extract(str(FIXTURES / "sample.jpg"))
        _check_base(result)
        assert result.schema["width_px"] > 0
        assert result.schema["height_px"] > 0
        assert result.schema["colour_mode"] is not None

    def test_extract_failure(self):
        result = ImageExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestVideoExtractor:
    def test_extract_mp4(self):
        result = VideoExtractor.extract(str(FIXTURES / "sample.mp4"))
        _check_base(result)
        assert isinstance(result.extraction_ms, int)

    def test_extract_failure(self):
        result = VideoExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestJsonExtractor:
    def test_extract_json(self):
        result = JsonExtractor.extract(str(FIXTURES / "sample.json"))
        _check_base(result)
        assert result.schema["top_level_type"] == "array"
        assert result.schema["key_count"] > 0
        assert result.schema["array_length"] == 3

    def test_extract_failure(self):
        result = JsonExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert result.error is not None


class TestGenericExtractor:
    def test_extract_bin(self):
        result = GenericExtractor.extract(str(FIXTURES / "sample.bin"))
        _check_base(result)
        assert list(result.schema.keys()) == ["filename", "file_size_bytes", "extension", "mime_type"]

    def test_extract_unknown(self):
        result = GenericExtractor.extract(str(FIXTURES / "sample.jpg"))
        _check_base(result)
        assert result.schema["mime_type"] == "image/jpeg"
