import os
from pathlib import Path


def read_file(path: str, allowed_paths: list = None, max_bytes: int = 10_485_760, allow_hidden: bool = False):
    if allowed_paths is None:
        allowed_paths = []

    file_path = Path(path).resolve()

    if not allow_hidden:
        for part in file_path.parts:
            if part.startswith("."):
                return {"error": f"Permission denied: hidden files are not allowed", "content": None}

    allowed = False
    for allowed_dir in allowed_paths:
        allowed_path = Path(allowed_dir).resolve()
        try:
            if str(file_path).startswith(str(allowed_path)):
                allowed = True
                break
        except ValueError:
            continue

    if not allowed:
        return {"error": f"Permission denied: {path} is not in allowed_paths", "content": None}

    if not file_path.exists():
        return {"error": f"File not found: {path}", "content": None}

    if file_path.is_dir():
        return {"error": f"Cannot read a directory: {path}", "content": None}

    try:
        size = file_path.stat().st_size
    except OSError as e:
        return {"error": str(e), "content": None}

    if size > max_bytes:
        return {"error": f"File too large: {size} bytes (max {max_bytes})", "content": None}

    try:
        import chardet
        raw = file_path.read_bytes()
        detected = chardet.detect(raw)
        encoding = detected.get("encoding", "utf-8") or "utf-8"
        content = raw.decode(encoding, errors="replace")
        return {"content": content, "encoding": encoding, "size": size, "mime": "text/plain"}
    except Exception as e:
        return {"error": f"Failed to read file: {e}", "content": None}
