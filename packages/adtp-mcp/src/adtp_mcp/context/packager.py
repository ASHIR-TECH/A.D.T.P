from __future__ import annotations

import base64
import logging
import pickle
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from adtp_mcp.context import ContextBlock
from adtp_mcp.context.detector import detect
from adtp_mcp.context.extractors import get_extractor
from adtp_mcp.context.history import get_history

logger = logging.getLogger("adtp-mcp.context.packager")

EXTRACTOR_TIMEOUT = 10


async def build_context(file_path: str | Path, session_id: str = "") -> dict[str, Any]:
    path = Path(file_path)
    start = time.perf_counter()

    try:
        mime_type = detect(str(path))
    except Exception:
        logger.exception("MIME detection failed for %s", path)
        return _empty_block(str(path.name))

    extractor_cls = get_extractor(mime_type)

    try:
        result = _extract_with_timeout(extractor_cls, str(path), EXTRACTOR_TIMEOUT)
    except TimeoutError:
        logger.warning("Extractor timed out (>%ds) for %s", EXTRACTOR_TIMEOUT, path)
        return _empty_block(str(path.name))
    except Exception:
        logger.exception("Extractor failed for %s", path)
        return _empty_block(str(path.name))

    history_entries = []
    if session_id:
        try:
            history_entries = get_history(session_id, mime_type)
        except Exception:
            logger.warning("History lookup failed for session %s", session_id)

    elapsed = int((time.perf_counter() - start) * 1000)

    block = ContextBlock(
        schema=result.schema,
        summary=result.summary,
        agent_hint=result.agent_hint,
        transfer_history=history_entries,
    )

    output = {
        "schema": block.schema,
        "summary": block.summary,
        "agent_hint": block.agent_hint,
        "transfer_history": [
            {
                "timestamp": e.timestamp,
                "mime_type": e.mime_type,
                "agent_action": e.agent_action,
            }
            for e in block.transfer_history
        ],
        "extraction_ms": elapsed,
    }

    logger.info("Context built for %s in %dms (mime=%s)", path.name, elapsed, mime_type)
    return output


def _extract_with_timeout(extractor_cls: type, file_path: str, timeout: int):
    child_code = """
import base64
import importlib
import pickle
import sys

module_name, class_name, file_path = sys.argv[1:4]
module = importlib.import_module(module_name)
extractor_cls = getattr(module, class_name)
result = extractor_cls.extract(file_path)
sys.stdout.write(base64.b64encode(pickle.dumps(result)).decode("ascii"))
"""
    try:
        completed = subprocess.run(
            [
                sys.executable,
                "-c",
                child_code,
                extractor_cls.__module__,
                extractor_cls.__name__,
                file_path,
            ],
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        raise TimeoutError(f"Extractor timed out after {timeout}s") from exc

    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or "Extractor subprocess failed")

    return pickle.loads(base64.b64decode(completed.stdout.encode("ascii")))


def _empty_block(filename: str) -> dict[str, Any]:
    return {
        "schema": None,
        "summary": None,
        "agent_hint": None,
        "transfer_history": [],
        "extraction_ms": 0,
    }
