from __future__ import annotations

import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class PdfExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            import fitz

            doc = fitz.open(str(file_path))
            page_count = doc.page_count
            metadata = doc.metadata or {}

            headings: list[str] = []
            first_page_text = ""
            has_images = False
            has_tables = False

            if page_count > 0:
                first_page = doc[0]
                blocks = first_page.get_text("dict").get("blocks", [])
                for block in blocks:
                    if block.get("type") == 0:
                        for line in block.get("lines", []):
                            for span in line.get("spans", []):
                                text = span.get("text", "").strip()
                                if text:
                                    font_size = span.get("size", 0)
                                    if font_size > 12:
                                        headings.append(text)
                                    if not first_page_text:
                                        first_page_text = text
                    elif block.get("type") == 1:
                        has_images = True

                first_page_text = (
                    first_page.get_text()[:500]
                    if hasattr(first_page, "get_text")
                    else ""
                )

                for page in doc:
                    tabs = page.find_tables()
                    if tabs.tables:
                        has_tables = True
                        break

            schema = {
                "page_count": page_count,
                "title": metadata.get("title", ""),
                "author": metadata.get("author", ""),
                "creator": metadata.get("creator", ""),
                "subject": metadata.get("subject", ""),
                "keywords": [
                    k.strip()
                    for k in metadata.get("keywords", "").split(",")
                    if k.strip()
                ],
                "has_images": has_images,
                "has_tables": has_tables,
                "first_page_text": first_page_text[:500],
            }

            summary = f"PDF document with {page_count} page(s)"
            if metadata.get("title"):
                summary += f" — {metadata['title']}"

            if page_count > 20:
                agent_hint = "Document is over 20 pages — summarise section by section"
            else:
                agent_hint = f"PDF has {page_count} page(s) — review content and extract key information"

            doc.close()

        except Exception as e:
            error = str(e)
            summary = "Failed to extract PDF metadata"
            agent_hint = "Binary file — process by filename and size only"

        elapsed = int((time.perf_counter() - start) * 1000)
        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
            error=error,
        )
