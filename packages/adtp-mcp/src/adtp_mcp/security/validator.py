from __future__ import annotations

import os
import uuid
from pathlib import Path
from typing import Iterable

from adtp_mcp.config import settings


BLOCKED_EXTENSIONS = {".exe", ".sh", ".bat", ".cmd"}
MAX_STRING_LENGTH = 4096


class ValidationError(ValueError):
    pass


def validate_string(value: str | None, field: str) -> None:
    if value is not None and len(value) > MAX_STRING_LENGTH:
        raise ValidationError(f"{field} exceeds {MAX_STRING_LENGTH} characters")


def validate_all_strings(fields: Iterable[tuple[str, str | None]]) -> None:
    for field, value in fields:
        validate_string(value, field)


def validate_absolute_path(path: str, field: str = "path") -> Path:
    validate_string(path, field)
    candidate = Path(path)
    if not candidate.is_absolute():
        raise ValidationError(f"{field} must be an absolute path")
    if ".." in candidate.parts:
        raise ValidationError(f"{field} must not contain path traversal")
    return candidate


def validate_file_for_send(file_path: str) -> Path:
    path = validate_absolute_path(file_path, "file_path")
    if not path.exists() or not path.is_file():
        raise ValidationError("file_path does not exist or is not a file")
    if path.suffix.lower() in BLOCKED_EXTENSIONS:
        raise ValidationError(f"blocked file extension: {path.suffix.lower()}")
    real = path.resolve()
    if ".." in real.parts:
        raise ValidationError("resolved file_path is invalid")
    size = os.stat(real).st_size
    if size > settings.max_file_size:
        raise ValidationError(f"file exceeds max size of {settings.max_file_size} bytes")
    return real


def validate_file_for_receive(file_path: str) -> Path:
    path = validate_absolute_path(file_path, "file_path")
    if not path.exists() or not path.is_file():
        raise ValidationError("file_path does not exist or is not a file")
    return path.resolve()


def validate_folder(folder_path: str) -> Path:
    path = validate_absolute_path(folder_path, "folder_path")
    if not path.is_dir():
        raise ValidationError("folder_path does not exist or is not a directory")
    return path.resolve()


def validate_uuid(value: str, field: str = "uuid") -> str:
    validate_string(value, field)
    try:
        parsed = uuid.UUID(value, version=4)
    except ValueError as exc:
        raise ValidationError(f"{field} must be a UUID v4") from exc
    if str(parsed) != value.lower():
        raise ValidationError(f"{field} must be canonical UUID format")
    return str(parsed)
