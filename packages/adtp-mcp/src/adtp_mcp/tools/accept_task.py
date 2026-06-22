from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge.tasks import accept_task as bridge_accept_task
from adtp_mcp.security.validator import validate_all_strings


class AcceptTaskInput(BaseModel):
    task_id: str


class AcceptTaskResult(BaseModel):
    task_id: str
    state: str


async def accept_task(input_data: AcceptTaskInput) -> dict:
    validate_all_strings((("task_id", input_data.task_id),))
    result = await bridge_accept_task(input_data.task_id)
    return AcceptTaskResult(
        task_id=result.get("task_id", input_data.task_id),
        state=result.get("state", "accepted"),
    ).model_dump()
