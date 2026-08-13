import uuid

import pytest

from adtp_mcp.security.validator import (
    ValidationError,
    validate_absolute_path,
    validate_file_for_send,
    validate_folder,
    validate_string,
    validate_uuid,
)


def test_validate_absolute_path_rejects_relative():
    with pytest.raises(ValidationError):
        validate_absolute_path("relative/file.txt")


def test_validate_absolute_path_rejects_traversal():
    with pytest.raises(ValidationError):
        validate_absolute_path("/tmp/../secret.txt")


def test_validate_file_for_send_accepts_regular_file(tmp_path):
    file_path = tmp_path / "report.txt"
    file_path.write_text("hello")
    assert validate_file_for_send(str(file_path)) == file_path.resolve()


def test_validate_file_for_send_blocks_executable_extension(tmp_path):
    file_path = tmp_path / "script.sh"
    file_path.write_text("echo nope")
    with pytest.raises(ValidationError):
        validate_file_for_send(str(file_path))


def test_validate_folder_accepts_directory(tmp_path):
    assert validate_folder(str(tmp_path)) == tmp_path.resolve()


def test_validate_uuid_v4():
    value = str(uuid.uuid4())
    assert validate_uuid(value, "transfer_id") == value


def test_validate_uuid_rejects_invalid():
    with pytest.raises(ValidationError):
        validate_uuid("not-a-uuid", "transfer_id")


def test_string_length_guard():
    with pytest.raises(ValidationError):
        validate_string("x" * 4097, "field")
