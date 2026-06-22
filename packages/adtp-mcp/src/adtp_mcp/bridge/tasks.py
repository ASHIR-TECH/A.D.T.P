from __future__ import annotations

from typing import Any

import httpx

API_BASE = "http://127.0.0.1:40003/api/v1/tasks"


async def create_task(
    peer_id: str,
    file_path: str,
    instructions: str,
    expected_artifacts: list[str] | None = None,
) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{API_BASE}",
            json={
                "peer_id": peer_id,
                "file_path": file_path,
                "instructions": instructions,
                "expected_artifacts": expected_artifacts or [],
            },
        )
        resp.raise_for_status()
        return resp.json()


async def accept_task(task_id: str) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(f"{API_BASE}/{task_id}/accept")
        resp.raise_for_status()
        return resp.json()


async def reject_task(task_id: str, reason: str) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{API_BASE}/{task_id}/reject",
            json={"reason": reason},
        )
        resp.raise_for_status()
        return resp.json()


async def report_artifact(
    task_id: str, file_path: str, artifact_name: str
) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{API_BASE}/{task_id}/artifact",
            json={
                "file_path": file_path,
                "artifact_name": artifact_name,
            },
        )
        resp.raise_for_status()
        return resp.json()


async def complete_task(task_id: str) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(f"{API_BASE}/{task_id}/complete")
        resp.raise_for_status()
        return resp.json()


async def fail_task(task_id: str, reason: str) -> dict[str, Any]:
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{API_BASE}/{task_id}/fail",
            json={"reason": reason},
        )
        resp.raise_for_status()
        return resp.json()
