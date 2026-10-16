from __future__ import annotations

from typing import Literal

from pydantic import BaseModel


class TransferRequest(BaseModel):
    file_path: str
    agent_name: str = "default"
    priority: Literal["normal", "urgent"] = "normal"
    message: str | None = None


class TransferResponse(BaseModel):
    transfer_id: str
    status: str
    filename: str
    queued_at: str | None = None
    estimated_ms: int = 0


class UploadResponse(BaseModel):
    """Result of POST /api/v1/transfers/upload (phone → desktop)."""

    id: str
    filename: str
    path: str
    size: int
    sha256: str
    content_type: str | None = None
    received_at: str


class TransferRecord(BaseModel):
    id: str
    session_id: str
    transfer_type: Literal["send", "push"]
    filename: str
    mime_type: str
    file_size: int
    status: Literal["pending", "in_progress", "delivered", "failed", "cancelled"]
    integrity_verified: bool = False
    signature: str = ""
    chain_hash: str = ""
    initiated_at: str
    completed_at: str | None = None
    context: dict | None = None


class TransferStatus(BaseModel):
    status: str
    bytes_sent: int = 0
    total_bytes: int = 0
    percent: float = 0.0
    elapsed_ms: int = 0


class TransferList(BaseModel):
    transfers: list[TransferRecord]
    total: int
    limit: int
    offset: int
