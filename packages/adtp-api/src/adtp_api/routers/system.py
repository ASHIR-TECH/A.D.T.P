from __future__ import annotations

import aiosqlite
from fastapi import APIRouter, Depends, Request

from adtp_api.dependencies import get_db
from adtp_api.models.system import SystemInfo
from adtp_api.services import system_service

router = APIRouter(prefix="/api/v1/system", tags=["system"])


@router.get("/info", response_model=SystemInfo)
async def system_info(
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    """Host telemetry for the paired desktop (best-effort and dependency-light).

    The mobile companion fetches this after pairing so the Home card and the
    AI's app-context carry the real machine name, OS, CPU, memory and battery instead of
    the placeholder values the pairing payload can't know.
    """
    socket_path = request.app.state.client_socket
    agent_socket = getattr(request.app.state, "agent_socket", socket_path)
    return await system_service.gather(db, agent_socket, socket_path)
