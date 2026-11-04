"""MIME detection for files crossing the phone ↔ desktop boundary.

Browsers, Android pickers and curl all disagree about `Content-Type`: some send
`application/octet-stream`, some omit it entirely. Falling back to the filename
keeps the transfer list, AI prompt and MARS chat tile correct for the formats
users actually send (images, PDFs, Office files, archives, media).
"""

from __future__ import annotations

import mimetypes
from pathlib import Path

GENERIC_TYPES = {
    "",
    "application/octet-stream",
    "binary/octet-stream",
    "multipart/form-data",
}

# `mimetypes` varies by OS (some Windows/Linux images miss these).
_EXTRA_TYPES = {
    ".md": "text/markdown",
    ".markdown": "text/markdown",
    ".json": "application/json",
    ".csv": "text/csv",
    ".yaml": "application/yaml",
    ".yml": "application/yaml",
    ".toml": "application/toml",
    ".log": "text/plain",
    ".doc": "application/msword",
    ".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ".xls": "application/vnd.ms-excel",
    ".xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ".ppt": "application/vnd.ms-powerpoint",
    ".pptx": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ".zip": "application/zip",
    ".gz": "application/gzip",
    ".tar": "application/x-tar",
    ".7z": "application/x-7z-compressed",
    ".rar": "application/vnd.rar",
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".heic": "image/heic",
    ".svg": "image/svg+xml",
    ".mp4": "video/mp4",
    ".mov": "video/quicktime",
    ".webm": "video/webm",
    ".mkv": "video/x-matroska",
    ".mp3": "audio/mpeg",
    ".wav": "audio/wav",
    ".m4a": "audio/mp4",
    ".ogg": "audio/ogg",
    ".apk": "application/vnd.android.package-archive",
}


def normalize_mime(value: str | None) -> str:
    """Drop parameters (`; charset=…`) and lowercase a declared Content-Type."""
    return (value or "").split(";", 1)[0].strip().lower()


def guess_mime(filename: str | None, declared: str | None = None) -> str:
    """Return the best MIME for a file, preferring a specific declared value."""
    declared_type = normalize_mime(declared)
    if declared_type not in GENERIC_TYPES:
        return declared_type

    name = Path(filename or "").name
    suffix = Path(name).suffix.lower()
    if suffix in _EXTRA_TYPES:
        return _EXTRA_TYPES[suffix]
    guessed = mimetypes.guess_type(name)[0]
    if guessed:
        return guessed
    return "application/octet-stream"


def is_text_mime(mime: str) -> bool:
    mime = normalize_mime(mime)
    return mime.startswith("text/") or mime in {
        "application/json",
        "application/xml",
        "application/yaml",
        "application/toml",
        "application/javascript",
    }
