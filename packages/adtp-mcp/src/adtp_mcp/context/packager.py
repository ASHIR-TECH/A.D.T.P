from __future__ import annotations

import asyncio
import logging
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

    loop = asyncio.get_running_loop()

    try:
        result = await asyncio.wait_for(
            loop.run_in_executor(None, extractor_cls.extract, str(path)),
            timeout=EXTRACTOR_TIMEOUT,
        )
    except asyncio.TimeoutError:
        logger.warning("Extractor timed out (>%ds) for %s", EXTRACTOR_TIMEOUT, path)
        return _empty_block(str(path.name))
    except Exception:
        logger.exception("Extractor failed for %s", path)
        return _empty_block(str(path.name))

    history_entries = []
    if session_id:
        try:
            history_entries = await loop.run_in_executor(
                None, get_history, session_id, mime_type
            )
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


def _empty_block(filename: str) -> dict[str, Any]:
    return {
        "schema": None,
        "summary": None,
        "agent_hint": None,
        "transfer_history": [],
        "extraction_ms": 0,
    }
