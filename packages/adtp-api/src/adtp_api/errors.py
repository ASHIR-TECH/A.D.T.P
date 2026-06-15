from __future__ import annotations

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse


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


def register_error_handlers(app: FastAPI) -> None:
    @app.exception_handler(AdtpError)
    async def adtp_error_handler(request: Request, exc: AdtpError):
        return JSONResponse(
            status_code=exc.status_code, content={"detail": exc.message}
        )
