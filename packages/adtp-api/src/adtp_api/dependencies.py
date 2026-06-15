from __future__ import annotations

from pathlib import Path

import aiosqlite
from fastapi import Request


async def get_db(request: Request) -> aiosqlite.Connection:
    db_path = Path(request.app.state.audit_db)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    db = await aiosqlite.connect(str(db_path))
    db.row_factory = aiosqlite.Row
    try:
        yield db
    finally:
        await db.close()
