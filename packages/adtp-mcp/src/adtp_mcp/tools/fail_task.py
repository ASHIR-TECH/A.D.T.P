from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge.tasks import fail_task as bridge_fail_task
from adtp_mcp.security.validator import validate_all_strings


class FailTaskInput(BaseModel):
    task_id: str
    reason: str


class FailTaskResult(BaseModel):
    task_id: str
    state: str
    reason: str


async def fail_task(input_data: FailTaskInput) -> dict:
    validate_all_strings(
        (
            ("task_id", input_data.task_id),
            ("reason", input_data.reason),
        )
    )
    result = await bridge_fail_task(input_data.task_id, input_data.reason)
    return FailTaskResult(
        task_id=result.get("task_id", input_data.task_id),
        state=result.get("state", "failed"),
        reason=input_data.reason,
    ).model_dump()
