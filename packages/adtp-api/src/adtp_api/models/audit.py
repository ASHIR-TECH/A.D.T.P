from __future__ import annotations

from typing import Literal

from pydantic import BaseModel


class AuditEntry(BaseModel):
    id: str
    session_id: str
    transfer_type: Literal["send", "push"]
    filename: str
    mime_type: str = ""
    file_size: int = 0
    status: str = ""
    integrity_verified: bool = False
    signature: str = ""
    chain_hash: str = ""
    initiated_at: str = ""
    completed_at: str | None = None


class AuditExport(BaseModel):
    csv_content: str


class AuditLogList(BaseModel):
    entries: list[AuditEntry]
    total: int


class ChainVerifyResult(BaseModel):
    valid: bool
    entries_checked: int
    first_failure_id: str | None = None
    verified_at: str
