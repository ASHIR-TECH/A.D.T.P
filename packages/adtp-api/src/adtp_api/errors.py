from __future__ import annotations

import logging

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse

logger = logging.getLogger("adtp_api.validation")

# Phone clients only show the status code, so a 422 without a reason is a
# dead end in the field. Name the offending field in the response and put the
# full detail in the gateway log.


class AdtpError(Exception):
    def __init__(self, message: str, status_code: int = 400):
        self.message = message
        self.status_code = status_code


class FileNotFoundError_(AdtpError):
    def __init__(self, path: str):
        super().__init__(f"Caution! file not found: {path}", 404)


class TransferNotFoundError(AdtpError):
    def __init__(self, transfer_id: str):
        super().__init__(f"Warning! Transfer not found: {transfer_id}", 404)


class DeviceNotFoundError(AdtpError):
    def __init__(self, device_id: str):
        super().__init__(f"Warning! Paired device not found: {device_id}", 404)


class SessionNotFoundError(AdtpError):
    def __init__(self, session_id: str):
        super().__init__(f"Be Warned! Session not found: {session_id}", 404)


class WatchNotFoundError(AdtpError):
    def __init__(self, watch_id: str):
        super().__init__(f"Halt! Watch not found: {watch_id}", 404)


class FileTooLargeError(AdtpError):
    def __init__(self, max_bytes: int):
        super().__init__(f"Hold! File exceeds limit of {max_bytes} bytes", 413)


class TransferInProgressError(AdtpError):
    def __init__(self):
        super().__init__("Patience! Transfer already delivered — cannot cancel", 409)


class DaemonUnreachableError(AdtpError):
    def __init__(self):
        super().__init__("Chill! Agent daemon unreachable — is adtp-client running?", 503)


class AuthenticationError(AdtpError):
    def __init__(self):
        super().__init__("Sorry! Unauthorized", 401)


def _describe_validation_error(error: dict) -> str:
    location = [
        str(part)
        for part in error.get("loc", ())
        if part not in ("body", "query", "path")
    ]
    field = ".".join(location) or "body"
    return f"{field}: {error.get('msg', 'invalid value')}"


def register_error_handlers(app: FastAPI) -> None:
    @app.exception_handler(RequestValidationError)
    async def validation_error_handler(request: Request, exc: RequestValidationError):
        errors = exc.errors()
        summary = "; ".join(_describe_validation_error(e) for e in errors)
        logger.warning(
            "Rejected %s %s from %s: %s",
            request.method,
            request.url.path,
            request.client.host if request.client else "unknown",
            summary or "invalid request body",
        )
        # Strip pydantic's non-JSON-serialisable `ctx` values, keep the parts a
        # client can act on.
        safe_errors = [
            {
                "loc": [str(part) for part in error.get("loc", ())],
                "msg": error.get("msg", "invalid value"),
                "type": error.get("type", ""),
            }
            for error in errors
        ]
        return JSONResponse(
            status_code=422,
            content={
                "detail": summary or "Invalid request body.",
                "errors": safe_errors,
            },
        )

    @app.exception_handler(AdtpError)
    async def adtp_error_handler(request: Request, exc: AdtpError):
        return JSONResponse(
            status_code=exc.status_code, content={"detail": exc.message}
        )
