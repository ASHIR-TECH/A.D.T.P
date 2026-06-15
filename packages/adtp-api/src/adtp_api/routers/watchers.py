from __future__ import annotations

from fastapi import APIRouter
from slowapi import Limiter
from slowapi.util import get_remote_address

from adtp_api.models.watcher import WatchRequest, WatchResponse

router = APIRouter(prefix="/api/v1/watchers", tags=["watchers"])
limiter = Limiter(key_func=get_remote_address)


@router.post("", status_code=201)
async def register_watch(body: WatchRequest):
    from adtp_mcp.bridge import client as bridge_client

    result = await bridge_client.register_watch(
        body.folder_path, body.file_pattern, body.trigger_action
    )
    return WatchResponse(
        watch_id=result.get("watch_id", ""),
        folder_path=body.folder_path,
        file_pattern=body.file_pattern,
        trigger_action=body.trigger_action,
        agent_name=body.agent_name,
    )


@router.get("")
async def list_watches():
    from adtp_mcp.bridge import client as bridge_client

    await bridge_client.get_transfers("all", 100)
    return {"watchers": [], "total": 0}


@router.get("/{watch_id}")
async def get_watch(watch_id: str):
    return WatchResponse(watch_id=watch_id, folder_path="", active=True)


@router.delete("/{watch_id}")
async def remove_watch(watch_id: str):
    from adtp_mcp.bridge import client as bridge_client

    await bridge_client.remove_watch(watch_id)
    return {"removed": True}
