import os
from pathlib import Path


def list_directory(path: str, allowed_paths: list = None):
    if allowed_paths is None:
        allowed_paths = []

    dir_path = Path(path).resolve()

    allowed = False
    for allowed_dir in allowed_paths:
        allowed_path = Path(allowed_dir).resolve()
        try:
            if str(dir_path).startswith(str(allowed_path)):
                allowed = True
                break
        except ValueError:
            continue

    if not allowed:
        return {"error": f"Permission denied: {path} is not in allowed_paths", "entries": []}

    if not dir_path.exists():
        return {"error": f"Directory not found: {path}", "entries": []}

    if not dir_path.is_dir():
        return {"error": f"Not a directory: {path}", "entries": []}

    entries = []
    try:
        for entry in dir_path.iterdir():
            try:
                stat = entry.stat()
                entries.append({
                    "name": entry.name,
                    "path": str(entry.resolve()),
                    "size": stat.st_size,
                    "modified": stat.st_mtime,
                    "is_dir": entry.is_dir(),
                })
            except (OSError, PermissionError):
                continue
    except PermissionError:
        return {"error": f"Permission denied listing directory: {path}", "entries": []}

    entries.sort(key=lambda e: (not e["is_dir"], e["name"].lower()))
    return {"entries": entries, "total": len(entries)}
