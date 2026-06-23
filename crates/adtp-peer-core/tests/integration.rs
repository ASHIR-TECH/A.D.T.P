use adtp_core::types::{PeerEntry, TransferRecord, TransferStatus, TransferType, TrustLevel, PEER_ID_SIZE};
use adtp_peer_core::artifact::Artifact;
use adtp_peer_core::audit::AuditLog;
use adtp_peer_core::identity::PeerIdentityKey;
use adtp_peer_core::task::{Task, TaskState, TaskStore};
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

#[tokio::test]
async fn test_full_task_lifecycle_integration() {
    let mut store = TaskStore::new();
    let sender = [1u8; PEER_ID_SIZE];
    let receiver = [2u8; PEER_ID_SIZE];
    let session_id = Uuid::new_v4();

    let task = Task::new(
        Uuid::new_v4(),
        session_id,
        sender,
        receiver,
        "Convert CSV to JSON".into(),
        vec!["output.json".into()],
    );
    let task_id = task.task_id;

    assert_eq!(task.state, TaskState::Received);
    store.add(task);

    let task = store.get_mut(&task_id.to_string()).unwrap();
    task.transition(TaskState::Accepted, None).unwrap();
    assert_eq!(task.state, TaskState::Accepted);

    let task = store.get_mut(&task_id.to_string()).unwrap();
    task.transition(TaskState::Processing, None).unwrap();
    assert_eq!(task.state, TaskState::Processing);

    let task = store.get_mut(&task_id.to_string()).unwrap();
    task.add_artifact(
        "output.json".into(),
        Uuid::new_v4(),
        "application/json".into(),
        4096,
    )
    .unwrap();
    assert_eq!(task.state, TaskState::ArtifactReady);

    let task = store.get_mut(&task_id.to_string()).unwrap();
    task.transition(TaskState::Completed, None).unwrap();
    assert!(task.state.is_terminal());

    let task = store.get(&task_id.to_string()).unwrap();
    assert_eq!(task.artifacts.len(), 1);
    assert_eq!(task.artifacts[0].artifact_name, "output.json");
}

#[tokio::test]
async fn test_task_rejection_flow() {
    let mut store = TaskStore::new();
    let task = Task::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        [1u8; PEER_ID_SIZE],
        [2u8; PEER_ID_SIZE],
        "Unwanted task".into(),
        vec![],
    );
    let task_id = task.task_id;
    store.add(task);

    let task = store.get_mut(&task_id.to_string()).unwrap();
    task.transition(TaskState::Rejected, Some("Not interested".into()))
        .unwrap();
    assert_eq!(task.state, TaskState::Rejected);
    assert_eq!(task.reason.as_deref(), Some("Not interested"));
    assert!(task.state.is_terminal());

    let result = store
        .get_mut(&task_id.to_string())
        .unwrap()
        .transition(TaskState::Accepted, None);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_artifact_links_to_transfer() {
    let task_id = Uuid::new_v4();
    let transfer_id = Uuid::new_v4();

    let artifact = Artifact::new(
        task_id,
        transfer_id,
        "result.csv".into(),
        "text/csv".into(),
        "/tmp/result.csv".into(),
        8192,
        [42u8; 32],
    );

    assert_eq!(artifact.task_id, task_id);
    assert_eq!(artifact.transfer_id, transfer_id);
    assert_eq!(artifact.file_size, 8192);
    assert_eq!(artifact.mime_type, "text/csv");
}

#[tokio::test]
async fn test_agent_card_sign_verify_integration() {
    use adtp_peer_core::agent_card::{build_unsigned_card, sign_card, verify_card};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let peer_id = [3u8; PEER_ID_SIZE];

    let unsigned = build_unsigned_card(
        &peer_id,
        &verifying_key.to_bytes(),
        Some("integration-peer".into()),
        "192.168.1.1",
        40001,
        1073741824,
        vec!["text/plain".into(), "application/json".into()],
        true,
        true,
        vec!["relay_peer_a".into()],
    );

    let card = sign_card(&unsigned, &signing_key);
    assert!(verify_card(&card).unwrap_or(false));
    assert_eq!(card.host, "192.168.1.1");
    assert_eq!(card.port, 40001);
    assert!(card.supports_tasks);
    assert!(card.supports_multihop);
    assert_eq!(card.relay_of.len(), 1);
}

#[test]
fn test_relay_path_loop_and_hop_limit() {
    use adtp_core::types::{RelayHop, PEER_ID_SIZE};
    use adtp_peer_core::relay::{build_relay_path, check_hop_limit, check_loop, target_reached};

    let hops = vec![
        RelayHop {
            peer_id: [1u8; PEER_ID_SIZE],
            host: "10.0.0.1".into(),
            port: 40001,
            timestamp_ns: 1000,
        },
        RelayHop {
            peer_id: [2u8; PEER_ID_SIZE],
            host: "10.0.0.2".into(),
            port: 40001,
            timestamp_ns: 2000,
        },
    ];

    let path = build_relay_path(hops, 4);

    assert!(check_loop(&path, &[1u8; PEER_ID_SIZE]).is_err());
    assert!(check_loop(&path, &[3u8; PEER_ID_SIZE]).is_ok());

    assert!(check_hop_limit(&path).is_ok());

    let limited_path = build_relay_path(
        vec![
            RelayHop {
                peer_id: [1u8; PEER_ID_SIZE],
                host: "10.0.0.1".into(),
                port: 40001,
                timestamp_ns: 1000,
            },
            RelayHop {
                peer_id: [2u8; PEER_ID_SIZE],
                host: "10.0.0.2".into(),
                port: 40001,
                timestamp_ns: 2000,
            },
        ],
        1,
    );
    assert!(check_hop_limit(&limited_path).is_err());

    assert!(target_reached(&path, &[2u8; PEER_ID_SIZE]));
    assert!(!target_reached(&path, &[3u8; PEER_ID_SIZE]));
}

#[tokio::test]
async fn test_tracer_multi_hop() {
    use adtp_core::types::TraceId;
    use adtp_peer_core::trace::Tracer;

    let mut tracer_a = Tracer::new("peer_a".to_string(), None);
    let mut tracer_b = Tracer::new("peer_b".to_string(), None);
    let trace_id = TraceId::new();

    let span_a = tracer_a.start_span(trace_id, "task-relay-1", "relay_receive", None);
    let span_a_id = span_a.span_id.clone();

    let span_b = tracer_b.start_span(
        trace_id,
        "task-relay-1",
        "relay_forward",
        Some(span_a.span_id.clone()),
    );
    let span_b_id = span_b.span_id.clone();

    assert_eq!(span_a.trace_id, span_b.trace_id);
    assert_eq!(span_b.parent_span_id.as_deref(), Some(span_a_id.as_str()));
    assert_eq!(span_a.peer_id, "peer_a");
    assert_eq!(span_b.peer_id, "peer_b");
    assert_eq!(span_b.operation, "relay_forward");

    tracer_a.end_span(&span_a_id);
    tracer_b.end_span(&span_b_id);

    assert!(tracer_a.span(&span_a_id).unwrap().ended_at.is_some());
    assert!(tracer_b.span(&span_b_id).unwrap().ended_at.is_some());
}

#[tokio::test]
async fn test_adtp_uri_parsing_integration() {
    use adtp_peer_core::relay::parse_adtp_uri;

    let uri = "adtp://a1b2c3d4@relay.example.com:40002/task/uuid-789";
    let parts = parse_adtp_uri(uri).unwrap();
    assert_eq!(parts.scheme, "adtp");
    assert_eq!(parts.peer_id, "a1b2c3d4");
    assert_eq!(parts.host, "relay.example.com");
    assert_eq!(parts.port, 40002);
    assert_eq!(parts.path, "/task/uuid-789");

    let uri_no_port = "adtp://deadbeef@10.0.0.5/transfer/xyz";
    let parts = parse_adtp_uri(uri_no_port).unwrap();
    assert_eq!(parts.port, 40001);
    assert_eq!(parts.path, "/transfer/xyz");
}

#[test]
fn test_relay_config_can_relay_for() {
    use adtp_peer_core::relay::RelayConfig;

    let config = RelayConfig {
        hop_limit: 4,
        relay_of: vec!["peer_a".into(), "peer_b".into()],
    };

    assert!(config.can_relay_for("peer_a"));
    assert!(config.can_relay_for("peer_b"));
    assert!(!config.can_relay_for("peer_c"));

    let open_config = RelayConfig::default();
    assert!(open_config.can_relay_for("anyone"));
}

#[test]
fn test_agent_card_server_serves_json() {
    use adtp_core::types::PEER_ID_SIZE;
    use adtp_peer_core::agent_card::{build_unsigned_card, sign_card, AgentCard};
    use adtp_peer_core::agent_card_server::AgentCardServer;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;


    let tokio_rt = tokio::runtime::Runtime::new().unwrap();

    tokio_rt.block_on(async {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let peer_id = [7u8; PEER_ID_SIZE];

        let unsigned = build_unsigned_card(
            &peer_id,
            &verifying_key.to_bytes(),
            Some("server-test".into()),
            "127.0.0.1",
            40001,
            1073741824,
            vec![],
            true,
            true,
            vec![],
        );
        let card = sign_card(&unsigned, &signing_key);
        let server = AgentCardServer::new(40003, card);

        let json = server.get_card_json().await;
        assert!(!json.is_empty());
        let parsed: AgentCard = serde_json::from_str(&json).unwrap();
        assert!(parsed.supports_tasks);
        assert!(parsed.supports_multihop);
    });
}
