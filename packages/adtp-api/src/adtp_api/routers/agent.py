from __future__ import annotations

import aiosqlite
from fastapi import APIRouter, Depends, Request

from adtp_api.dependencies import get_db
from adtp_api.models.agent import AgentMessageCreated, AgentMessageCreate, AgentStatusResponse
from adtp_api.services import agent_service

router = APIRouter(prefix="/api/v1/agent", tags=["agent"])


@router.get("/status", response_model=AgentStatusResponse)
async def agent_status(
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    socket_path = request.app.state.client_socket
    agent_socket = getattr(request.app.state, "agent_socket", socket_path)
    return await agent_service.get_status(db, agent_socket, socket_path)


@router.post("/message", status_code=201, response_model=AgentMessageCreated)
async def create_message(
    body: AgentMessageCreate,
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    message_id = await agent_service.submit_message(
        db,
        request.app.state.audit_db,
        getattr(request.app.state, "agent_socket", request.app.state.client_socket),
        request.app.state.client_socket,
        body.message,
        body.conversation_id,
    )
    return {"id": message_id}


@router.get("/messages/{message_id}")
async def get_message(
    message_id: str,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await agent_service.get_message(db, message_id)