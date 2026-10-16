from __future__ import annotations

import asyncio
import hmac
import json
import os
from datetime import datetime, timezone

from fastapi import FastAPI, WebSocket, WebSocketDisconnect

from adtp_api.config import settings
from adtp_api.services import agent_service, system_service

# How long a single chat turn may run before the phone is told it timed out.
CHAT_TIMEOUT_SECS = 120.0
POLL_INTERVAL_SECS = 0.75


def _expected_token() -> str:
    return os.getenv("ADTP_API_TOKEN", settings.api_token)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _device_from(identity: dict, desktop_id: str | None = None) -> dict:
    """Mars `state_update` device shape (see mobile src/relay/types.ts).

    The phone sends the desktop id it paired with, so the desktop is reported
    under that identity. Without it the gateway falls back to the hostname,
    which would make the phone render a second card for the same machine (the
    libp2p peer id the QR carried is not derivable from the gateway's env).
    """
    hostname = identity.get("hostname") or "Desktop"
    return {
        "id": desktop_id or hostname,
        "name": hostname,
        "os": identity.get("os") or "Desktop",
        "status": "online",
        "lastSeen": "Just Now",
    }


class RelaySession:
    """One mobile client attached to the LAN relay.

    Implements the MARS wire protocol the phone's RelayClient already speaks:
    authenticate → auth_ack, then chat_message → chat_response, plus periodic
    state_update frames so the Home grid shows this desktop.
    """

    def __init__(self, app: FastAPI, websocket: WebSocket):
        self.app = app
        self.ws = websocket
        self.send_lock = asyncio.Lock()
        self.tasks: set[asyncio.Task] = set()
        self.desktop_id: str | None = None

    async def send(self, payload: dict) -> None:
        async with self.send_lock:
            await self.ws.send_json(payload)

    async def run(self) -> None:
        await self.ws.accept()
        if not await self._authenticate():
            return

        await self.send({"type": "auth_ack"})
        await self._push_state()

        state_task = asyncio.create_task(self._state_loop())
        try:
            while True:
                raw = await self.ws.receive_text()
                await self._handle(raw)
        except WebSocketDisconnect:
            pass
        except Exception:
            pass
        finally:
            state_task.cancel()
            for task in list(self.tasks):
                task.cancel()

    async def _authenticate(self) -> bool:
        try:
            raw = await self.ws.receive_text()
            message = json.loads(raw)
        except (WebSocketDisconnect, ValueError, TypeError):
            await self._close_quietly()
            return False

        params = message.get("params") if isinstance(message, dict) else None
        token = ""
        desktop_id: str | None = None
        if isinstance(params, dict):
            token = str(params.get("token") or "")
            raw_id = params.get("desktopId") or params.get("nodeId")
            if isinstance(raw_id, str) and raw_id and raw_id != "__auth__":
                desktop_id = raw_id
        expected = _expected_token()
        is_auth = (
            isinstance(message, dict)
            and message.get("type") == "command"
            and message.get("action") == "authenticate"
        )
        if not is_auth or not expected or not hmac.compare_digest(token, expected):
            await self.send({"type": "auth_rejected", "reason": "invalid pairing token"})
            await self._close_quietly()
            return False
        self.desktop_id = desktop_id
        return True

    async def _close_quietly(self) -> None:
        try:
            await self.ws.close()
        except Exception:
            pass

    async def _push_state(self) -> None:
        await self.send(
            {
                "type": "state_update",
                "devices": [
                    _device_from(system_service.identity(), self.desktop_id)
                ],
            }
        )

    async def _state_loop(self) -> None:
        interval = max(1.0, float(settings.relay_state_interval))
        while True:
            await asyncio.sleep(interval)
            await self._push_state()

    async def _handle(self, raw: str) -> None:
        try:
            message = json.loads(raw)
        except (ValueError, TypeError):
            return
        if not isinstance(message, dict):
            return

        if message.get("type") == "chat_message":
            task = asyncio.create_task(self._respond(message))
            self.tasks.add(task)
            task.add_done_callback(self.tasks.discard)

    async def _respond(self, message: dict) -> None:
        session_id = str(message.get("sessionId") or "")
        text = str(message.get("text") or "").strip()
        if not text:
            return

        reply_text, provider = await self._run_agent(text, session_id or None)
        await self.send(
            {
                "type": "chat_response",
                "sessionId": session_id,
                "text": reply_text,
                "timestamp": _now(),
                "providerLabel": provider,
            }
        )

    async def _run_agent(self, text: str, conversation_id: str | None) -> tuple[str, str | None]:
        """Submit to the desktop's embedded agent and poll until it finishes."""
        db_path = self.app.state.audit_db
        agent_socket = getattr(self.app.state, "agent_socket", self.app.state.client_socket)
        daemon_socket = self.app.state.client_socket

        db = await agent_service.open_db(db_path)
        try:
            message_id = await agent_service.submit_message(
                db, db_path, agent_socket, daemon_socket, text, conversation_id
            )
            deadline = asyncio.get_running_loop().time() + CHAT_TIMEOUT_SECS
            while asyncio.get_running_loop().time() < deadline:
                state = await agent_service.get_message(db, message_id)
                status = state.get("status")
                if status == "completed":
                    return state.get("text") or "Done.", state.get("provider")
                if status == "failed":
                    return state.get("error") or "The desktop agent failed.", state.get("provider")
                await asyncio.sleep(POLL_INTERVAL_SECS)
            return "The desktop agent is still working. Please try again.", None
        except Exception as exc:  # noqa: BLE001 - report failures to the phone
            return f"Desktop agent unreachable: {exc}", None
        finally:
            await db.close()


def register_relay(app: FastAPI) -> None:
    @app.websocket("/relay")
    async def relay_endpoint(websocket: WebSocket) -> None:
        await RelaySession(app, websocket).run()
