from __future__ import annotations

from typing import Any

from adtp_mcp.context.extractors.audio import AudioExtractor
from adtp_mcp.context.extractors.csv import CsvExtractor
from adtp_mcp.context.extractors.excel import ExcelExtractor
from adtp_mcp.context.extractors.generic import GenericExtractor
from adtp_mcp.context.extractors.image import ImageExtractor
from adtp_mcp.context.extractors.json import JsonExtractor
from adtp_mcp.context.extractors.pdf import PdfExtractor
from adtp_mcp.context.extractors.video import VideoExtractor
from adtp_mcp.context.extractors.word import WordExtractor

REGISTRY: dict[str, type[Any]] = {
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": ExcelExtractor,
    "application/vnd.ms-excel": ExcelExtractor,
    "application/pdf": PdfExtractor,
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document": WordExtractor,
    "text/csv": CsvExtractor,
    "text/tab-separated-values": CsvExtractor,
    "audio/mpeg": AudioExtractor,
    "audio/wav": AudioExtractor,
    "audio/flac": AudioExtractor,
    "audio/ogg": AudioExtractor,
    "video/mp4": VideoExtractor,
    "video/x-matroska": VideoExtractor,
    "video/quicktime": VideoExtractor,
    "image/jpeg": ImageExtractor,
    "image/png": ImageExtractor,
    "image/webp": ImageExtractor,
    "image/gif": ImageExtractor,
    "application/json": JsonExtractor,
    "application/ld+json": JsonExtractor,
}


def get_extractor(mime_type: str) -> type[Any]:
    return REGISTRY.get(mime_type, GenericExtractor)
