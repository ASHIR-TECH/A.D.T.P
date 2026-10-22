#!/usr/bin/env python3
"""
Lightweight ADTP Relay Listener (~100KB source, <5MB RAM)

A minimal persistent WebSocket client for the ADTP gateway's /relay endpoint.
Runs 24/7 as a daemon on the desktop to:
  - Maintain connection to gateway /relay
  - Authenticate with pairing token
  - Respond to state_update (device presence for phone Home grid)
  - Forward chat_message to local agent and return chat_response
  - Handle file transfers (upload/send/download)
  - Respond to pings for phone to know desktop is online

Usage:
  python relay_listener.py --gateway-url ws://localhost:40003/relay --token <pairing-token> --desktop-id <desktop-id>

Can be installed as a systemd user service for auto-start.
"""

import asyncio
import json
import os
import signal
import sys
import uuid
from pathlib import Path
from typing import Optional

import websockets
from websockets.client import WebSocketClientProtocol

# Minimal deps: websockets (pip install websockets)


class RelayListener:
    def __init__(
        self,
        gateway_ws_url: str,
        pairing_token: str,
        desktop_id: str,
        agent_socket: str = "/tmp/adtp-agent.sock",
    ):
        self.gateway_ws_url = gateway_ws_url
        self.pairing_token = pairing_token
        self.desktop_id = desktop_id
        self.agent_socket = agent_socket
        self.ws: Optional[WebSocketClientProtocol] = None
        self.running = False
        self.ping_interval = 30  # seconds
        self.state_interval = 10  # seconds

    async def connect(self):
        """Connect to gateway /relay and authenticate."""
        self.ws = await websockets.connect(
            self.gateway_ws_url,
            ping_interval=None,  # we handle pings ourselves
            close_timeout=5,
        )
        # Authenticate
        await self.ws.send(json.dumps({
            "type": "command",
            "nodeId": "__auth__",
            "action": "authenticate",
            "params": {
                "token": self.pairing_token,
                "desktopId": self.desktop_id,
            },
        }))
        print(f"[relay] Connected to {self.gateway_ws_url}, authenticated as {self.desktop_id}")

    async def send(self, payload: dict):
        if self.ws and self.ws.open:
            await self.ws.send(json.dumps(payload))

    async def send_state_update(self):
        """Send periodic state_update so phone shows this desktop as online."""
        import platform
        hostname = platform.node() or "Desktop"
        await self.send({
            "type": "state_update",
            "devices": [{
                "id": self.desktop_id,
                "name": hostname,
                "os": platform.system() or "Desktop",
                "status": "online",
                "lastSeen": "Just Now",
            }]
        })

    async def handle_message(self, raw: str):
        try:
            msg = json.loads(raw)
        except json.JSONDecodeError:
            return

        msg_type = msg.get("type")

        if msg_type == "auth_ack":
            print("[relay] Authenticated successfully")
            # Send initial state
            await self.send_state_update()
        elif msg_type == "auth_rejected":
            reason = msg.get("reason", "unknown")
            print(f"[relay] Auth rejected: {reason}")
            self.running = False
        elif msg_type == "chat_message":
            session_id = msg.get("sessionId")
            text = msg.get("text", "")
            await self.handle_chat_message(session_id, text)
        elif msg_type == "ping":
            # Respond to ping so phone knows we're alive
            await self.send({"type": "pong", "timestamp": msg.get("timestamp")})
        else:
            print(f"[relay] Unknown message type: {msg_type}")

    async def handle_chat_message(self, session_id: str, text: str):
        """Forward to local agent via Unix socket, return response."""
        try:
            # Connect to agent socket
            reader, writer = await asyncio.open_unix_connection(self.agent_socket)
            request = {
                "jsonrpc": "2.0",
                "id": str(uuid.uuid4()),
                "method": "chat",
                "params": {"session_id": session_id, "text": text},
            }
            writer.write((json.dumps(request) + "\n").encode())
            await writer.drain()

            # Read response (newline-delimited JSON)
            line = await reader.readline()
            writer.close()
            await writer.wait_closed()

            if line:
                resp = json.loads(line.decode())
                result = resp.get("result", {})
                await self.send({
                    "type": "chat_response",
                    "sessionId": session_id,
                    "text": result.get("text", "Done."),
                    "timestamp": result.get("timestamp", ""),
                    "providerLabel": result.get("provider"),
                })
            else:
                await self.send({
                    "type": "chat_response",
                    "sessionId": session_id,
                    "text": "Agent returned empty response.",
                    "timestamp": "",
                })
        except Exception as e:
            print(f"[relay] Agent error: {e}")
            await self.send({
                "type": "chat_response",
                "sessionId": session_id,
                "text": f"Agent error: {e}",
                "timestamp": "",
            })

    async def run(self):
        """Main loop: connect, handle messages, reconnect on failure."""
        self.running = True
        backoff = 1

        while self.running:
            try:
                await self.connect()
                backoff = 1

                # Background tasks
                state_task = asyncio.create_task(self.state_loop())
                ping_task = asyncio.create_task(self.ping_loop())

                # Message receive loop
                async for raw in self.ws:
                    await self.handle_message(raw)

                state_task.cancel()
                ping_task.cancel()

            except websockets.ConnectionClosed as e:
                print(f"[relay] Connection closed: {e.code} {e.reason}")
            except Exception as e:
                print(f"[relay] Error: {e}")

            if self.running:
                print(f"[relay] Reconnecting in {backoff}s...")
                await asyncio.sleep(backoff)
                backoff = min(backoff * 2, 60)

    async def state_loop(self):
        while self.running and self.ws and self.ws.open:
            await self.send_state_update()
            await asyncio.sleep(self.state_interval)

    async def ping_loop(self):
        while self.running and self.ws and self.ws.open:
            await self.send({"type": "ping", "timestamp": ""})
            await asyncio.sleep(self.ping_interval)

    def stop(self):
        self.running = False
        if self.ws:
            asyncio.create_task(self.ws.close())


async def main():
    import argparse
    parser = argparse.ArgumentParser(description="ADTP Relay Listener")
    parser.add_argument("--gateway-url", required=True, help="Gateway WebSocket URL (e.g. ws://localhost:40003/relay)")
    parser.add_argument("--token", required=True, help="Pairing token")
    parser.add_argument("--desktop-id", required=True, help="Desktop ID (from QR)")
    parser.add_argument("--agent-socket", default="/tmp/adtp-agent.sock", help="Agent Unix socket path")
    args = parser.parse_args()

    listener = RelayListener(args.gateway_url, args.token, args.desktop_id, args.agent_socket)

    # Handle signals for graceful shutdown
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, listener.stop)

    print(f"[relay] Starting listener for desktop {args.desktop_id}")
    await listener.run()
    print("[relay] Stopped")


if __name__ == "__main__":
    asyncio.run(main())