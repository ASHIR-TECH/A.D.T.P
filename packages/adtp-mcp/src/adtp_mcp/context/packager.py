from __future__ import annotations

from pathlib import Path
from typing import Any


def build_context(file_path: str | Path) -> dict[str, Any]:
    path = Path(file_path)
    return {
        "schema": None,
        "summary": None,
        "agent_hint": None,
        "transfer_history": [],
        "custom_metadata": {"phase": "2", "filename": path.name},
    }
