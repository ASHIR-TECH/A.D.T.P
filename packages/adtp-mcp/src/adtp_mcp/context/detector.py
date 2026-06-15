from __future__ import annotations

import mimetypes
from pathlib import Path


class AdtpContextError(Exception):
    pass


def detect(file_path: str | Path) -> str:
    path = Path(file_path)
    if not path.exists():
        raise AdtpContextError(f"File not found: {file_path}")
    try:
        import magic

        mime = magic.from_file(str(path), mime=True)
        if mime and mime != "application/octet-stream":
            return str(mime)
    except (ImportError, FileNotFoundError, PermissionError):
        pass

    mime = mimetypes.guess_type(str(path))[0]
    return mime or "application/octet-stream"
