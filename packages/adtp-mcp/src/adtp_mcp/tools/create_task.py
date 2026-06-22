from __future__ import annotations

from pydantic import BaseModel, Field

from adtp_mcp.bridge.tasks import create_task as bridge_create_task
from adtp_mcp.security.validator import validate_all_strings


class CreateTaskInput(BaseModel):
    peer_id: str
    file_path: str
    instructions: str
    expected_artifacts: list[str] = Field(default_factory=list)


class CreateTaskResult(BaseModel):
    task_id: str
    state: str


async def create_task(input_data: CreateTaskInput) -> dict:
    validate_all_strings(
        (
            ("peer_id", input_data.peer_id),
            ("file_path", input_data.file_path),
            ("instructions", input_data.instructions),
        )
    )
    result = await bridge_create_task(
        input_data.peer_id,
        input_data.file_path,
        input_data.instructions,
        input_data.expected_artifacts,
    )
    return CreateTaskResult(
        task_id=result.get("task_id", ""),
        state=result.get("state", "received"),
    ).model_dump()
