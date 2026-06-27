import os
import glob as glob_mod
from pathlib import Path


def find_file(pattern: str, directory: str = None, extension: str = None, allowed_paths: list = None):
    if allowed_paths is None:
        allowed_paths = []

    search_dir = directory or os.path.expanduser("~")
    search_path = Path(search_dir).resolve()

    allowed = _is_path_allowed(search_path, allowed_paths)
    if not allowed:
        return {"error": f"Permission denied: {search_dir} is not in allowed_paths", "results": []}

    results = []
    matches = list(glob_mod.glob(os.path.join(search_path, pattern), recursive=True))
    matches += list(glob_mod.glob(os.path.join(search_path, "**", pattern), recursive=True))
    matches = list(dict.fromkeys(matches))

    for match in matches:
        match_path = Path(match).resolve()
        if not _is_path_allowed(match_path, allowed_paths):
            continue

        if extension and not match.endswith(extension):
            continue

        try:
            stat = match_path.stat()
            results.append({
                "path": str(match_path),
                "name": match_path.name,
                "size": stat.st_size,
                "modified": stat.st_mtime,
                "is_dir": match_path.is_dir(),
            })
        except (OSError, PermissionError):
            continue

    return {"results": results, "total": len(results)}


def _is_path_allowed(path: Path, allowed_paths: list) -> bool:
    if not allowed_paths:
        return False
    path_str = str(path.resolve())

    if ".." in path_str.split(os.sep):
        return False

    for allowed in allowed_paths:
        allowed_path = Path(allowed).resolve()
        try:
            path_str.startswith(str(allowed_path))
            return True
        except ValueError:
            continue
    return False
