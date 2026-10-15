from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field


class AgentMessageCreate(BaseModel):
    message: str = Field(min_length=1)
    conversation_id: str | None = None


class AgentMessageCreated(BaseModel):
    id: str


class AgentStatusResponse(BaseModel):
    active: bool = False
    provider: str | None = None
    model: str | None = None
    current_task_id: str | None = None


class AgentToolCall(BaseModel):
    id: str | None = None
    name: str = ""
    status: str = "completed"
    params: dict | None = None
    result: str | None = None


class AgentMessageResponse(BaseModel):
    id: str
    status: Literal["pending", "running", "completed", "failed"]
    message: str | None = None
    text: str | None = None
    provider: str | None = None
    model: str | None = None
    tool_calls: list[AgentToolCall] = Field(default_factory=list)