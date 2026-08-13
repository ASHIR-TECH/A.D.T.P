from __future__ import annotations

import hmac
import os

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from adtp_api.config import settings


EXEMPT_PATHS = {"/api/v1/health", "/api/v1/health/ready", "/api/v1/health/queue"}


def register_auth_middleware(app: FastAPI) -> None:
    @app.middleware("http")
    async def auth_middleware(request: Request, call_next):
        path = request.url.path
        if (
            path in EXEMPT_PATHS
            or path.startswith("/docs")
            or path.startswith("/redoc")
            or path.startswith("/openapi")
        ):
            return await call_next(request)

        auth_header = request.headers.get("Authorization", "")
        if not auth_header.startswith("Bearer "):
            return JSONResponse(status_code=401, content={"detail": "Unauthorized"})

        token = auth_header.removeprefix("Bearer ")
        expected = os.getenv("ADTP_API_TOKEN", settings.api_token)

        if not expected or len(token) != len(expected):
            return JSONResponse(status_code=401, content={"detail": "Unauthorized"})

        if not hmac.compare_digest(token, expected):
            return JSONResponse(status_code=401, content={"detail": "Unauthorized"})

        return await call_next(request)
