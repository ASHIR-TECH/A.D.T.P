from __future__ import annotations

import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult
from adtp_mcp.context.detector import detect


class GenericExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        path = Path(file_path)
        mime = detect(str(path))
        stat = path.stat()

        schema = {
            "filename": path.name,
            "file_size_bytes": stat.st_size,
            "extension": path.suffix,
            "mime_type": mime,
        }

        summary = f"Binary file {path.name} ({stat.st_size} bytes, {mime})"
        agent_hint = "Binary file — process by filename and size only"
        elapsed = int((time.perf_counter() - start) * 1000)

        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
        )
