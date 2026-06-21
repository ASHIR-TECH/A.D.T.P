use adtp_core::types::{PeerEntry, TransferRecord, TransferStatus, TransferType, TrustLevel, PEER_ID_SIZE};
use adtp_peer_core::audit::AuditLog;
use adtp_peer_core::identity::PeerIdentityKey;
use adtp_peer_core::trust::TrustStore;
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("adtp-peer-test-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn test_identity_generate_and_load() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("identity.key");

    let id1 = PeerIdentityKey::load_or_generate(&path).unwrap();
    assert!(!id1.peer_id.iter().all(|&b| b == 0));
    assert_eq!(id1.peer_id.len(), PEER_ID_SIZE);

    let id2 = PeerIdentityKey::load_or_generate(&path).unwrap();
    assert_eq!(id1.peer_id, id2.peer_id);
    assert_eq!(
        id1.verifying_key.to_bytes(),
        id2.verifying_key.to_bytes()
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_trust_store_add_list_remove() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("peers.toml");

    let mut store = TrustStore::load(&path);
    assert!(store.list_peers().is_empty());

    store.add_peer(PeerEntry {
        peer_id: "abc123".to_string(),
        public_key: String::new(),
        nickname: Some("test-peer".to_string()),
        host: "127.0.0.1".to_string(),
        port: 40001,
        trust_level: TrustLevel::Trusted,
    });

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].peer_id, "abc123");

    store.remove_peer("abc123");
    assert!(store.list_peers().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_trust_store_persistence() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("peers.toml");

    {
        let mut store = TrustStore::load(&path);
        store.add_peer(PeerEntry {
            peer_id: "def456".to_string(),
            public_key: String::new(),
            nickname: None,
            host: "10.0.0.1".to_string(),
            port: 40002,
            trust_level: TrustLevel::Trusted,
        });
        store.save().unwrap();
    }

    {
        let store = TrustStore::load(&path);
        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_id, "def456");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_trust_is_allowed() {
    let mut peer_id = [0u8; PEER_ID_SIZE];
    peer_id[..6].copy_from_slice(b"abcdef");

    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("peers.toml");

    let store = TrustStore::load(&path);
    assert!(!store.is_allowed(&peer_id));

    let mut store = TrustStore::load(&path);
    store.add_peer(PeerEntry {
        peer_id: hex::encode(peer_id),
        public_key: String::new(),
        nickname: None,
        host: "".to_string(),
        port: 0,
        trust_level: TrustLevel::Trusted,
    });

    assert!(store.is_allowed(&peer_id));

    store.set_trust(&hex::encode(peer_id), TrustLevel::Blocked);
    assert!(!store.is_allowed(&peer_id));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_audit_log_transfer_and_verify() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("audit.db");

    let audit = AuditLog::open(&path).unwrap();

    let transfer_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let record = TransferRecord {
        id: transfer_id,
        session_id,
        sender_peer_id: [1u8; PEER_ID_SIZE],
        receiver_peer_id: [2u8; PEER_ID_SIZE],
        transfer_type: TransferType::Transfer,
        file_meta: adtp_core::types::FileMeta {
            filename: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            file_size: 100,
            extension: "txt".to_string(),
            last_modified: None,
            content_hash: [3u8; 32],
        },
        status: TransferStatus::Delivered,
        initiated_at: Utc::now(),
        completed_at: Some(Utc::now()),
        signature: "sig".to_string(),
        received_hash: Some([4u8; 32]),
        integrity_verified: true,
    };

    audit.log_session_start(&session_id, "127.0.0.1:40001").unwrap();
    audit.log_transfer(&record).unwrap();
    audit.log_session_end(&session_id, 1).unwrap();

    assert!(audit.verify_chain().unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_audit_chain_tamper_detection() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("audit.db");

    let audit = AuditLog::open(&path).unwrap();

    let session_id = Uuid::new_v4();
    audit.log_session_start(&session_id, "127.0.0.1:40001").unwrap();

    for i in 0..3 {
        let record = TransferRecord {
            id: Uuid::new_v4(),
            session_id,
            sender_peer_id: [i as u8; PEER_ID_SIZE],
            receiver_peer_id: [2u8; PEER_ID_SIZE],
            transfer_type: TransferType::Transfer,
            file_meta: adtp_core::types::FileMeta {
                filename: format!("file{i}.txt"),
                mime_type: "text/plain".to_string(),
                file_size: 100,
                extension: "txt".to_string(),
                last_modified: None,
                content_hash: [3u8; 32],
            },
            status: TransferStatus::Delivered,
            initiated_at: Utc::now(),
            completed_at: Some(Utc::now()),
            signature: "sig".to_string(),
            received_hash: Some([4u8; 32]),
            integrity_verified: true,
        };
        audit.log_transfer(&record).unwrap();
    }

    assert!(audit.verify_chain().unwrap());

    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE transfers SET filename = 'tampered.txt' WHERE filename = 'file1.txt'",
        [],
    )
    .unwrap();
    drop(conn);

    assert!(!audit.verify_chain().unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}
