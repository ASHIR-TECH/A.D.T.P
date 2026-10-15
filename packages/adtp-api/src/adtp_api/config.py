from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    # Bind address for the REST gateway. Set ADTP_API_HOST="0.0.0.0" so the
    # mobile companion can reach the desktop gateway over the LAN. The default
    # stays 127.0.0.1 (loopback only) for safety — the QR pairing payload,
    # however, advertises the non-loopback IP and assumes the gateway binds
    # 0.0.0.0. Env vars use the ADTP_ prefix: api_host -> ADTP_API_HOST,
    # api_port -> ADTP_API_PORT, api_token -> ADTP_API_TOKEN.
    api_host: str = "127.0.0.1"
    api_port: int = 40003
    api_token: str = ""
    client_socket: str = "/tmp/adtp-client.sock"
    server_socket: str = "/tmp/adtp-server.sock"
    # Socket where the desktop app exposes its embedded agent (AgentManager).
    # adtp-api prefers this for /api/v1/agent/* so mobile chat reaches the real
    # tool loop; it falls back to client_socket when the desktop isn't running.
    agent_socket: str = "/tmp/adtp-agent.sock"
    audit_db: str = str(Path.home() / ".config" / "adtp" / "client-audit.db")
    cors_origins: str = "http://localhost:*"
    rate_limit: str = "100/minute"
    api_docs: bool = True
    log_level: Literal["DEBUG", "INFO", "WARNING", "ERROR"] = "INFO"
    max_file_size: int = 10 * 1024 * 1024 * 1024

    model_config = {
        "env_prefix": "ADTP_",
        "env_file": ".env",
        "env_file_encoding": "utf-8",
        "extra": "ignore",
    }


settings = Settings()
