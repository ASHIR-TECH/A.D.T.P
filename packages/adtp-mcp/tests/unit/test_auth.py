import pytest

from adtp_mcp.security.auth import AuthError, validate_token


def test_stdio_mode_bypasses_auth():
    assert validate_token("short", None, stdio_mode=True)


def test_valid_bearer_token_passes():
    token = "x" * 32
    assert validate_token(token, f"Bearer {token}")


def test_missing_token_fails():
    with pytest.raises(AuthError):
        validate_token("x" * 32, None)


def test_short_configured_token_fails():
    with pytest.raises(AuthError):
        validate_token("too-short", "Bearer too-short")
