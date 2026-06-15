from __future__ import annotations

import argparse
import json
from typing import Any, Callable

from adtp_mcp.config import settings
from adtp_mcp.resources import pending_files, session_status, transfer_log
from adtp_mcp.tools import (
    GetFileContextInput,
    ListTransfersInput,
    ReceiveFileInput,
    SendFileInput,
    TransferStatusInput,
    UnwatchFolderInput,
    WatchFolderInput,
    get_file_context,
    list_transfers,
    receive_file,
    send_file,
    transfer_status,
    unwatch_folder,
    watch_folder,
)

ToolHandler = Callable[[Any], Any]

TOOLS: dict[str, tuple[type, ToolHandler]] = {
    "send_file": (SendFileInput, send_file),
    "receive_file": (ReceiveFileInput, receive_file),
    "watch_folder": (WatchFolderInput, watch_folder),
    "unwatch_folder": (UnwatchFolderInput, unwatch_folder),
    "list_transfers": (ListTransfersInput, list_transfers),
    "get_file_context": (GetFileContextInput, get_file_context),
    "transfer_status": (TransferStatusInput, transfer_status),
}

RESOURCES = {
    transfer_log.URI: transfer_log.read,
    session_status.URI: session_status.read,
    pending_files.URI: pending_files.read,
}


def configure_logging() -> None:
    try:
        import structlog
    except ImportError:
        return

    structlog.configure(
        processors=[
            structlog.processors.TimeStamper(fmt="iso"),
            structlog.processors.JSONRenderer(),
        ],
    )


async def call_tool(name: str, arguments: dict[str, Any]) -> Any:
    if name not in TOOLS:
        raise ValueError(f"Unknown tool: {name}")
    model, handler = TOOLS[name]
    input_data = model(**arguments)
    return await handler(input_data)


async def read_resource(uri: str) -> Any:
    if uri not in RESOURCES:
        raise ValueError(f"Unknown resource: {uri}")
    return await RESOURCES[uri]()


def build_fastmcp():
    try:
        from mcp.server.fastmcp import FastMCP
    except ImportError as exc:
        raise RuntimeError(
            "The official MCP SDK is required. Install with: uv sync"
        ) from exc

    mcp = FastMCP("adtp")

    @mcp.tool()
    async def send_file_tool(
        file_path: str,
        agent_name: str = "default",
        message: str | None = None,
        priority: str = "normal",
    ) -> dict:
        return await send_file(
            SendFileInput(
                file_path=file_path,
                agent_name=agent_name,
                message=message,
                priority=priority,
            )
        )

    @mcp.tool()
    async def receive_file_tool(
        file_path: str,
        destination_folder: str | None = None,
        message: str | None = None,
    ) -> dict:
        return await receive_file(
            ReceiveFileInput(
                file_path=file_path,
                destination_folder=destination_folder,
                message=message,
            )
        )

    @mcp.tool()
    async def watch_folder_tool(
        folder_path: str,
        file_pattern: str | None = None,
        trigger_action: str | None = None,
    ) -> dict:
        return await watch_folder(
            WatchFolderInput(
                folder_path=folder_path,
                file_pattern=file_pattern,
                trigger_action=trigger_action,
            )
        )

    @mcp.tool()
    async def unwatch_folder_tool(
        watch_id: str | None = None, folder_path: str | None = None
    ) -> dict:
        return await unwatch_folder(
            UnwatchFolderInput(watch_id=watch_id, folder_path=folder_path)
        )

    @mcp.tool()
    async def list_transfers_tool(status: str = "all", limit: int = 10) -> list[dict]:
        return await list_transfers(ListTransfersInput(status=status, limit=limit))

    @mcp.tool()
    async def get_file_context_tool(transfer_id: str) -> dict:
        return await get_file_context(GetFileContextInput(transfer_id=transfer_id))

    @mcp.tool()
    async def transfer_status_tool(transfer_id: str) -> dict:
        return await transfer_status(TransferStatusInput(transfer_id=transfer_id))

    @mcp.resource(transfer_log.URI)
    async def transfer_log_resource() -> str:
        return json.dumps(await transfer_log.read())

    @mcp.resource(session_status.URI)
    async def session_status_resource() -> str:
        return json.dumps(await session_status.read())

    @mcp.resource(pending_files.URI)
    async def pending_files_resource() -> str:
        return json.dumps(await pending_files.read())

    return mcp


def main() -> None:
    try:
        from dotenv import load_dotenv
    except ImportError:
        load_dotenv = None

    if load_dotenv is not None:
        load_dotenv()
    configure_logging()
    parser = argparse.ArgumentParser(description="ADTP MCP server")
    parser.add_argument(
        "--http", action="store_true", help="Run Streamable HTTP transport"
    )
    parser.add_argument("--port", type=int, default=settings.mcp_port)
    parser.add_argument("--dev", action="store_true", help="Development mode")
    args = parser.parse_args()

    mcp = build_fastmcp()
    if args.http:
        mcp.settings.port = args.port
        mcp.run(transport="streamable-http")
    else:
        mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
