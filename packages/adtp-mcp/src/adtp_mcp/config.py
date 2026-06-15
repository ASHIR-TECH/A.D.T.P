from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


DEFAULT_MAX_FILE_SIZE = 10 * 1024 * 1024 * 1024


@dataclass(frozen=True)
class Settings:
    client_socket: Path
    server_socket: Path
    mcp_port: int
    bridge_timeout_ms: int
    log_level: str
    max_file_size: int
    auth_token: str | None

    @classmethod
    def from_env(cls) -> "Settings":
        return cls(
            client_socket=Path(
                os.getenv("ADTP_CLIENT_SOCKET", "/tmp/adtp-client.sock")
            ),
            server_socket=Path(
                os.getenv("ADTP_SERVER_SOCKET", "/tmp/adtp-server.sock")
            ),
            mcp_port=int(os.getenv("ADTP_MCP_PORT", "40002")),
            bridge_timeout_ms=int(os.getenv("ADTP_BRIDGE_TIMEOUT_MS", "5000")),
            log_level=os.getenv("ADTP_LOG_LEVEL", "INFO"),
            max_file_size=int(
                os.getenv("ADTP_MAX_FILE_SIZE", str(DEFAULT_MAX_FILE_SIZE))
            ),
            auth_token=os.getenv("ADTP_AUTH_TOKEN"),
        )


settings = Settings.from_env()
