from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge.tasks import complete_task as bridge_complete_task
from adtp_mcp.security.validator import validate_all_strings


class CompleteTaskInput(BaseModel):
    task_id: str


class CompleteTaskResult(BaseModel):
    task_id: str
    state: str


async def complete_task(input_data: CompleteTaskInput) -> dict:
    validate_all_strings((("task_id", input_data.task_id),))
    result = await bridge_complete_task(input_data.task_id)
    return CompleteTaskResult(
        task_id=result.get("task_id", input_data.task_id),
        state=result.get("state", "completed"),
    ).model_dump()
