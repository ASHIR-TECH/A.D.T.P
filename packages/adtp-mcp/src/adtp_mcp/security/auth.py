from __future__ import annotations

import hmac


class AuthError(PermissionError):
    pass


def validate_token(
    expected_token: str | None, provided_header: str | None, *, stdio_mode: bool = False
) -> bool:
    if stdio_mode or expected_token is None:
        return True
    if len(expected_token) < 32:
        raise AuthError("Configured auth token is too short")
    if not provided_header or not provided_header.startswith("Bearer "):
        raise AuthError("Unauthorized")
    provided = provided_header.removeprefix("Bearer ").strip()
    if not hmac.compare_digest(expected_token, provided):
        raise AuthError("Unauthorized")
    return True
