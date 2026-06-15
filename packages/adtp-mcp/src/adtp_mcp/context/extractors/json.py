from __future__ import annotations

import json as json_mod
import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class JsonExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            path = Path(file_path)
            raw = path.read_text(encoding="utf-8", errors="replace")
            data = json_mod.loads(raw)

            top_level_type = (
                "object"
                if isinstance(data, dict)
                else "array"
                if isinstance(data, list)
                else "scalar"
            )
            key_count = 0
            array_length = 0
            inferred_schema: dict = {}
            has_nulls = False
            max_depth = 1

            if isinstance(data, dict):
                key_count = len(data)
                has_nulls = any(v is None for v in data.values())
                for k, v in data.items():
                    inferred_schema[k] = JsonExtractor._json_type(v)
            elif isinstance(data, list):
                array_length = len(data)
                if data:
                    if all(isinstance(item, dict) for item in data):
                        all_keys: set[str] = set()
                        for item in data:
                            all_keys.update(item.keys())
                        key_count = len(all_keys)
                        for k in all_keys:
                            vals = [
                                item.get(k) for item in data if isinstance(item, dict)
                            ]
                            types = {JsonExtractor._json_type(v) for v in vals}
                            inferred_schema[k] = ", ".join(sorted(types))
                        has_nulls = any(
                            v is None
                            for item in data
                            for v in (
                                item.values() if isinstance(item, dict) else [item]
                            )
                        )

            max_depth = JsonExtractor._depth(data)

            from jsonschema import Draft202012Validator

            try:
                validator = Draft202012Validator(
                    {
                        "type": "object",
                        "properties": {
                            k: {"type": v} for k, v in inferred_schema.items()
                        },
                    }
                )
                schema_obj = validator.schema
            except Exception:
                schema_obj = inferred_schema

            schema = {
                "top_level_type": top_level_type,
                "key_count": key_count,
                "array_length": array_length,
                "inferred_schema": schema_obj,
                "depth": max_depth,
                "has_nulls": has_nulls,
            }

            if top_level_type == "array":
                summary = f"JSON array with {array_length} item(s), {key_count} key(s), depth {max_depth}"
                agent_hint = "Top-level structure is an array — treat as a dataset"
            else:
                summary = f"JSON object with {key_count} key(s), depth {max_depth}"
                agent_hint = (
                    f"JSON has {key_count} top-level keys — parse according to schema"
                )

        except Exception as e:
            error = str(e)
            summary = "Failed to extract JSON metadata"
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
    def _json_type(val) -> str:
        if val is None:
            return "null"
        if isinstance(val, bool):
            return "boolean"
        if isinstance(val, int):
            return "integer"
        if isinstance(val, float):
            return "number"
        if isinstance(val, str):
            return "string"
        if isinstance(val, list):
            return "array"
        if isinstance(val, dict):
            return "object"
        return "unknown"

    @staticmethod
    def _depth(obj, current: int = 0) -> int:
        if isinstance(obj, dict) and obj:
            return max(JsonExtractor._depth(v, current + 1) for v in obj.values())
        if isinstance(obj, list) and obj:
            return max(JsonExtractor._depth(item, current + 1) for item in obj)
        return current
