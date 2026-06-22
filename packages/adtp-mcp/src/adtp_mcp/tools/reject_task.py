from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge.tasks import reject_task as bridge_reject_task
from adtp_mcp.security.validator import validate_all_strings


class RejectTaskInput(BaseModel):
    task_id: str
    reason: str = "Declined"


class RejectTaskResult(BaseModel):
    task_id: str
    state: str
    reason: str


async def reject_task(input_data: RejectTaskInput) -> dict:
    validate_all_strings(
        (
            ("task_id", input_data.task_id),
            ("reason", input_data.reason),
        )
    )
    result = await bridge_reject_task(input_data.task_id, input_data.reason)
    return RejectTaskResult(
        task_id=result.get("task_id", input_data.task_id),
        state=result.get("state", "rejected"),
        reason=input_data.reason,
    ).model_dump()
