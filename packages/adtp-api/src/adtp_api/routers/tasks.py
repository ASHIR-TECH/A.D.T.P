from __future__ import annotations

from fastapi import APIRouter, Depends, Query

import aiosqlite

from adtp_api.dependencies import get_db
from adtp_api.models.task import (
    ArtifactRequest,
    TaskActionRequest,
    TaskCreateRequest,
    TaskList,
)
from adtp_api.services import task_service

router = APIRouter(prefix="/api/v1/tasks", tags=["tasks"])


@router.post("", status_code=201)
async def create_task(
    body: TaskCreateRequest,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.create_task(
        db, body.peer_id, body.file_path, body.instructions, body.expected_artifacts
    )


@router.get("")
async def list_tasks(
    peer_id: str | None = Query(None),
    state: str | None = Query(None),
    limit: int = Query(20, ge=1, le=100),
    offset: int = Query(0, ge=0),
    db: aiosqlite.Connection = Depends(get_db),
) -> TaskList:
    return await task_service.list_tasks(db, peer_id, state, limit, offset)


@router.get("/{task_id}")
async def get_task(
    task_id: str,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.get_task(db, task_id)


@router.post("/{task_id}/accept")
async def accept_task(task_id: str, db: aiosqlite.Connection = Depends(get_db)) -> dict:
    return await task_service.transition_task(db, task_id, "accepted")


@router.post("/{task_id}/reject")
async def reject_task(
    task_id: str,
    body: TaskActionRequest,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.transition_task(db, task_id, "rejected", body.reason)


@router.post("/{task_id}/complete")
async def complete_task(
    task_id: str,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.transition_task(db, task_id, "completed")


@router.post("/{task_id}/fail")
async def fail_task(
    task_id: str,
    body: TaskActionRequest,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.transition_task(db, task_id, "failed", body.reason)


@router.post("/{task_id}/artifact")
async def report_artifact(
    task_id: str,
    body: ArtifactRequest,
    db: aiosqlite.Connection = Depends(get_db),
) -> dict:
    return await task_service.report_artifact(
        db, task_id, body.file_path, body.artifact_name
    )
