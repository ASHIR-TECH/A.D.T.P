from __future__ import annotations

import sys
from pathlib import Path

import aiosqlite
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from adtp_api.config import settings
from adtp_api.errors import register_error_handlers
from adtp_api.middleware.auth import register_auth_middleware
from adtp_api.middleware.ratelimit import register_rate_limit
from adtp_api.routers import audit, health, sessions, tasks, transfers, watchers


def create_app() -> FastAPI:
    if not settings.api_token:
        print("FATAL: ADTP_API_TOKEN is required", file=sys.stderr)
        sys.exit(1)
    if len(settings.api_token) < 32:
        print("FATAL: ADTP_API_TOKEN must be at least 32 characters", file=sys.stderr)
        sys.exit(1)

    docs_enabled = settings.api_docs
    app = FastAPI(
        title="ADTP REST API",
        version="0.1.0",
        docs_url="/docs" if docs_enabled else None,
        redoc_url="/redoc" if docs_enabled else None,
    )

    origins = [o.strip() for o in settings.cors_origins.split(",") if o.strip()]
    app.add_middleware(
        CORSMiddleware,
        allow_origins=origins,
        allow_methods=["GET", "POST", "DELETE", "OPTIONS"],
        allow_headers=["Authorization", "Content-Type"],
    )

    app.state.audit_db = settings.audit_db
    app.state.client_socket = settings.client_socket
    app.state.server_socket = settings.server_socket

    register_error_handlers(app)
    register_auth_middleware(app)
    register_rate_limit(app)

    @app.on_event("startup")
    async def init_tasks_db():
        db_path = Path(settings.audit_db)
        db_path.parent.mkdir(parents=True, exist_ok=True)
        db = await aiosqlite.connect(str(db_path))
        await db.execute(
            """CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                transfer_id TEXT NOT NULL,
                session_id TEXT NOT NULL DEFAULT '',
                sender_peer_id TEXT NOT NULL DEFAULT '',
                receiver_peer_id TEXT NOT NULL DEFAULT '',
                instructions TEXT NOT NULL DEFAULT '',
                expected_artifacts TEXT NOT NULL DEFAULT '[]',
                state TEXT NOT NULL DEFAULT 'received',
                reason TEXT,
                artifacts_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"""
        )
        await db.commit()
        await db.close()

    app.include_router(health.router)
    app.include_router(transfers.router)
    app.include_router(sessions.router)
    app.include_router(watchers.router)
    app.include_router(audit.router)
    app.include_router(tasks.router)

    return app


app = create_app()


def main() -> None:
    import uvicorn

    uvicorn.run(
        "adtp_api.main:app",
        host=settings.api_host,
        port=settings.api_port,
        log_level=settings.log_level.lower(),
    )


if __name__ == "__main__":
    main()
