from __future__ import annotations

from pydantic import BaseModel


class ArtifactModel(BaseModel):
    artifact_name: str
    transfer_id: str
    mime_type: str = "application/octet-stream"
    file_size: int = 0
    produced_at: str = ""


class TaskRecord(BaseModel):
    task_id: str
    transfer_id: str
    session_id: str = ""
    sender_peer_id: str = ""
    receiver_peer_id: str = ""
    instructions: str = ""
    expected_artifacts: list[str] = []
    state: str = "received"
    reason: str | None = None
    artifacts: list[ArtifactModel] = []
    created_at: str = ""
    updated_at: str = ""


class TaskList(BaseModel):
    tasks: list[TaskRecord]
    total: int
    limit: int
    offset: int


class TaskCreateRequest(BaseModel):
    peer_id: str
    file_path: str
    instructions: str
    expected_artifacts: list[str] = []


class TaskActionRequest(BaseModel):
    reason: str = ""


class ArtifactRequest(BaseModel):
    file_path: str
    artifact_name: str
