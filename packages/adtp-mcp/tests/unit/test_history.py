from __future__ import annotations

import os
import sqlite3
import tempfile
from pathlib import Path

import pytest

from adtp_mcp.context.history import get_history


@pytest.fixture
def audit_db():
    tmp = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    db_path = tmp.name
    tmp.close()

    conn = sqlite3.connect(db_path)
    conn.execute(
        """
        CREATE TABLE transfer_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            mime_type TEXT,
            timestamp TEXT,
            agent_action TEXT,
            file_hash TEXT
        )
        """
    )
    conn.execute(
        "INSERT INTO transfer_audit (session_id, mime_type, timestamp, agent_action) VALUES (?, ?, ?, ?)",
        ("session-1", "application/pdf", "2024-01-01T10:00:00", "processed"),
    )
    conn.execute(
        "INSERT INTO transfer_audit (session_id, mime_type, timestamp, agent_action) VALUES (?, ?, ?, ?)",
        ("session-1", "application/pdf", "2024-01-01T11:00:00", "summarised"),
    )
    conn.execute(
        "INSERT INTO transfer_audit (session_id, mime_type, timestamp, agent_action) VALUES (?, ?, ?, ?)",
        ("session-1", "image/jpeg", "2024-01-01T12:00:00", ""),
    )
    conn.execute(
        "INSERT INTO transfer_audit (session_id, mime_type, timestamp, agent_action) VALUES (?, ?, ?, ?)",
        ("session-2", "application/pdf", "2024-01-02T10:00:00", ""),
    )
    conn.commit()
    conn.close()

    old_env = os.environ.get("ADTP_AUDIT_DB")
    os.environ["ADTP_AUDIT_DB"] = db_path
    yield db_path
    if old_env is None:
        del os.environ["ADTP_AUDIT_DB"]
    else:
        os.environ["ADTP_AUDIT_DB"] = old_env
    Path(db_path).unlink(missing_ok=True)


class TestHistory:
    def test_returns_matching_entries(self, audit_db):
        entries = get_history("session-1", "application/pdf")
        assert len(entries) == 2
        assert entries[0].mime_type == "application/pdf"
        assert entries[0].timestamp == "2024-01-01T11:00:00"
        assert entries[0].agent_action == "summarised"

    def test_respects_session_boundary(self, audit_db):
        entries = get_history("session-2", "application/pdf")
        assert len(entries) == 1
        assert entries[0].timestamp == "2024-01-02T10:00:00"

    def test_no_match_returns_empty(self, audit_db):
        entries = get_history("session-1", "text/csv")
        assert entries == []

    def test_mime_filter(self, audit_db):
        entries = get_history("session-1", "image/jpeg")
        assert len(entries) == 1
        assert entries[0].mime_type == "image/jpeg"

    def test_max_five(self, audit_db):
        conn = sqlite3.connect(audit_db)
        for i in range(10):
            conn.execute(
                "INSERT INTO transfer_audit (session_id, mime_type, timestamp) VALUES (?, ?, ?)",
                ("session-1", "text/csv", f"2024-01-01T{10 + i:02d}:00:00"),
            )
        conn.commit()
        conn.close()
        entries = get_history("session-1", "text/csv")
        assert len(entries) <= 5

    def test_no_db_env(self):
        old = os.environ.pop("ADTP_AUDIT_DB", None)
        try:
            entries = get_history("session-1", "application/pdf")
            assert entries == []
        finally:
            if old is not None:
                os.environ["ADTP_AUDIT_DB"] = old
