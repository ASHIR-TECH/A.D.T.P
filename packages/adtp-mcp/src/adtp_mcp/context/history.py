from __future__ import annotations

import os
import sqlite3
from pathlib import Path

from adtp_mcp.context import TransferHistoryEntry


def get_history(session_id: str, mime_type: str) -> list[TransferHistoryEntry]:
    db_path = os.getenv("ADTP_AUDIT_DB")
    if not db_path:
        return []

    path = Path(db_path)
    if not path.exists():
        return []

    try:
        conn = sqlite3.connect(str(path))
        conn.row_factory = sqlite3.Row
        cursor = conn.execute(
            """
            SELECT timestamp, mime_type, agent_action
            FROM transfer_audit
            WHERE mime_type = ? AND session_id = ?
            ORDER BY timestamp DESC
            LIMIT 5
            """,
            (mime_type, session_id),
        )
        results = [
            TransferHistoryEntry(
                timestamp=str(row["timestamp"]),
                mime_type=str(row["mime_type"]),
                agent_action=str(row["agent_action"] or ""),
            )
            for row in cursor.fetchall()
        ]
        conn.close()
        return results
    except sqlite3.Error:
        return []
