from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    api_port: int = 40003
    api_host: str = "127.0.0.1"
    api_token: str = ""
    client_socket: str = "/tmp/adtp-client.sock"
    server_socket: str = "/tmp/adtp-server.sock"
    audit_db: str = str(Path.home() / ".config" / "adtp" / "client-audit.db")
    cors_origins: str = "http://localhost:*"
    rate_limit: str = "100/minute"
    api_docs: bool = True
    log_level: Literal["DEBUG", "INFO", "WARNING", "ERROR"] = "INFO"
    max_file_size: int = 10 * 1024 * 1024 * 1024

    model_config = {"env_prefix": "ADTP_"}


settings = Settings()
