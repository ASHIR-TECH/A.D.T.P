use adtp_core::{
    errors::{AdtpError, AdtpResult},
    integrity::{compute_chain_hash, genesis_hash},
    types::{TransferRecord, BLAKE3_HASH_SIZE},
};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

pub struct AuditLog {
    conn: Mutex<Connection>,
}

impl AuditLog {
    pub fn open(path: &Path) -> AdtpResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(internal_error)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS transfers (
                id              TEXT PRIMARY KEY,
                session_id      TEXT NOT NULL,
                sender_peer_id  TEXT NOT NULL DEFAULT '',
                receiver_peer_id TEXT NOT NULL DEFAULT '',
                transfer_type   TEXT NOT NULL,
                filename        TEXT NOT NULL,
                mime_type       TEXT NOT NULL,
                file_size       INTEGER NOT NULL,
                status          TEXT NOT NULL,
                signature       TEXT NOT NULL,
                content_hash    TEXT NOT NULL,
                integrity_ok    INTEGER NOT NULL DEFAULT 0,
                initiated_at    TEXT NOT NULL,
                completed_at    TEXT,
                chain_hash      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id              TEXT PRIMARY KEY,
                peer_addr       TEXT NOT NULL,
                started_at      TEXT NOT NULL,
                ended_at        TEXT,
                transfer_count  INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS chain_state (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                last_hash       TEXT NOT NULL
            );
            ",
        )
        .map_err(internal_error)?;
        conn.execute(
            "INSERT OR IGNORE INTO chain_state (id, last_hash) VALUES (1, ?1)",
            params![hex::encode(genesis_hash())],
        )
        .map_err(internal_error)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn log_transfer(&self, record: &TransferRecord) -> AdtpResult<()> {
        let previous_hash = self.previous_chain_hash()?;
        let initiated_at = record.initiated_at.to_rfc3339();
        let entry_data = chain_entry_data(
            &record.id.to_string(),
            &record.session_id.to_string(),
            &record.file_meta.filename,
            &initiated_at,
        );
        let chain_hash = compute_chain_hash(&previous_hash, entry_data.as_bytes());
        let chain_hash_hex = hex::encode(chain_hash);

        let conn = self.conn.lock().map_err(internal_error)?;
        conn.execute(
            "INSERT INTO transfers (
                    id, session_id, sender_peer_id, receiver_peer_id, transfer_type,
                    filename, mime_type, file_size, status, signature, content_hash,
                    integrity_ok, initiated_at, completed_at, chain_hash
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                record.id.to_string(),
                record.session_id.to_string(),
                hex::encode(record.sender_peer_id),
                hex::encode(record.receiver_peer_id),
                format!("{:?}", record.transfer_type),
                record.file_meta.filename,
                record.file_meta.mime_type,
                record.file_meta.file_size as i64,
                format!("{:?}", record.status),
                record.signature,
                hex::encode(record.file_meta.content_hash),
                record.integrity_verified as i32,
                initiated_at,
                record.completed_at.map(|d| d.to_rfc3339()),
                chain_hash_hex.clone(),
            ],
        )
        .map_err(internal_error)?;
        conn.execute(
            "UPDATE chain_state SET last_hash = ?1 WHERE id = 1",
            params![chain_hash_hex],
        )
        .map_err(internal_error)?;

        tracing::info!("Audit: logged transfer {}", record.id);
        Ok(())
    }

    pub fn log_session_start(&self, session_id: &Uuid, peer_addr: &str) -> AdtpResult<()> {
        let conn = self.conn.lock().map_err(internal_error)?;
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, peer_addr, started_at) VALUES (?1, ?2, ?3)",
            params![session_id.to_string(), peer_addr, Utc::now().to_rfc3339()],
        )
        .map_err(internal_error)?;
        Ok(())
    }

    pub fn log_session_end(&self, session_id: &Uuid, transfer_count: usize) -> AdtpResult<()> {
        let conn = self.conn.lock().map_err(internal_error)?;
        conn.execute(
            "UPDATE sessions SET ended_at=?1, transfer_count=?2 WHERE id=?3",
            params![
                Utc::now().to_rfc3339(),
                transfer_count as i64,
                session_id.to_string()
            ],
        )
        .map_err(internal_error)?;
        Ok(())
    }

    pub fn verify_chain(&self) -> AdtpResult<bool> {
        let mut current_hash = genesis_hash();
        let conn = self.conn.lock().map_err(internal_error)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, filename, initiated_at, chain_hash
                 FROM transfers ORDER BY rowid ASC",
            )
            .map_err(internal_error)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(internal_error)?;

        for row in rows {
            let (id, session_id, filename, initiated_at, stored_hash) =
                row.map_err(internal_error)?;
            let entry_data = chain_entry_data(&id, &session_id, &filename, &initiated_at);
            let computed = compute_chain_hash(&current_hash, entry_data.as_bytes());

            if hex::encode(computed) != stored_hash {
                tracing::error!("Chain verification FAILED at transfer {}", id);
                return Ok(false);
            }
            current_hash = computed;
        }

        tracing::info!("Audit chain verification PASSED");
        Ok(true)
    }

    fn previous_chain_hash(&self) -> AdtpResult<[u8; BLAKE3_HASH_SIZE]> {
        let conn = self.conn.lock().map_err(internal_error)?;
        let previous_hash_hex: String = conn
            .query_row(
                "SELECT last_hash FROM chain_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(internal_error)?;
        let previous_hash = hex::decode(previous_hash_hex).map_err(internal_error)?;
        let previous_hash: [u8; BLAKE3_HASH_SIZE] =
            previous_hash
                .try_into()
                .map_err(|bytes: Vec<u8>| AdtpError::Internal {
                    reason: format!("Invalid audit chain hash length: {}", bytes.len()),
                })?;
        Ok(previous_hash)
    }
}

fn chain_entry_data(id: &str, session_id: &str, filename: &str, initiated_at: &str) -> String {
    format!("{id}{session_id}{filename}{initiated_at}")
}

fn internal_error(error: impl std::fmt::Display) -> AdtpError {
    AdtpError::Internal {
        reason: error.to_string(),
    }
}
