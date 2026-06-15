from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class TransferHistoryEntry:
    timestamp: str
    mime_type: str
    agent_action: str = ""


@dataclass
class ExtractionResult:
    schema: dict[str, Any]
    summary: str
    agent_hint: str
    extraction_ms: int
    error: str | None = None


@dataclass
class ContextBlock:
    schema: dict[str, Any] | None = None
    summary: str | None = None
    agent_hint: str | None = None
    transfer_history: list[TransferHistoryEntry] = field(default_factory=list)
