from __future__ import annotations

import csv
import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class CsvExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            path = Path(file_path)
            raw = path.read_bytes()

            import chardet

            detected = chardet.detect(raw)
            encoding = detected.get("encoding", "utf-8") or "utf-8"

            try:
                text = raw.decode(encoding)
            except (UnicodeDecodeError, LookupError):
                text = raw.decode("utf-8", errors="replace")
                encoding = "utf-8"

            sniffer = csv.Sniffer()
            try:
                dialect = sniffer.sniff(text[:4096])
                delimiter = dialect.delimiter
                if delimiter and ord(delimiter) < 32:
                    raise ValueError(f"Invalid delimiter: {ord(delimiter)}")
            except (csv.Error, ValueError):
                delimiter = ","

            reader = csv.reader(text.splitlines(), delimiter=delimiter)
            rows = list(reader)

            if not rows:
                raise ValueError("Empty or unparseable CSV content")

            header_row = rows[0] if rows else []
            data_rows = rows[1:] if len(rows) > 1 else []

            # Validate: header should not be longer than 200 entries for reasonable CSV
            if len(header_row) > 200:
                raise ValueError(
                    f"Excessive columns ({len(header_row)}) — likely not valid CSV"
                )

            # Check for excessive control characters in header
            header_text = " ".join(header_row)
            ctrl_count = sum(
                1 for c in header_text if ord(c) < 32 and c not in "\t\n\r"
            )
            if len(header_text) > 0 and ctrl_count > max(10, len(header_text) * 0.3):
                raise ValueError(
                    "Header contains excessive control characters — likely binary data"
                )

            detected_types: dict[str, str] = {}
            has_null_values = False

            for i, col_name in enumerate(header_row):
                col_values = []
                for row in data_rows:
                    if i < len(row):
                        val = row[i].strip()
                        if val == "" or val.lower() in ("null", "none", "na", "n/a"):
                            has_null_values = True
                            col_values.append(None)
                        else:
                            col_values.append(val)

                if col_values:
                    non_null = [v for v in col_values if v is not None]
                    detected_types[col_name] = CsvExtractor._infer_type(non_null)

            schema = {
                "delimiter": delimiter,
                "encoding": encoding,
                "header_row": header_row,
                "row_count": len(data_rows),
                "column_count": len(header_row),
                "detected_types": detected_types,
                "has_null_values": has_null_values,
            }

            summary = f"CSV with {len(data_rows)} row(s), {len(header_row)} column(s)"
            if detected_types:
                date_cols = [k for k, v in detected_types.items() if v == "date"]
                if date_cols:
                    agent_hint = f"Date columns detected: {', '.join(date_cols[:3])} — check for time-series patterns"
                else:
                    agent_hint = f"Dataset with {len(header_row)} features and {len(data_rows)} observations"

        except Exception as e:
            error = str(e)
            summary = "Failed to extract CSV metadata"
            agent_hint = "Binary file — process by filename and size only"

        elapsed = int((time.perf_counter() - start) * 1000)
        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
            error=error,
        )

    @staticmethod
    def _infer_type(values: list[str]) -> str:
        int_count = 0
        float_count = 0
        date_count = 0
        total = min(len(values), 100)

        for v in values[:100]:
            v = v.strip()
            try:
                int(v)
                int_count += 1
                continue
            except (ValueError, TypeError):
                pass
            try:
                float(v)
                float_count += 1
                continue
            except (ValueError, TypeError):
                pass
            import re

            if re.match(r"\d{4}[-/]\d{1,2}[-/]\d{1,2}", v):
                date_count += 1

        if total == 0:
            return "string"
        if date_count / total > 0.5:
            return "date"
        if int_count / total > 0.5:
            return "integer"
        if (int_count + float_count) / total > 0.5:
            return "numeric"
        return "string"
