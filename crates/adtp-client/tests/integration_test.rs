use adtp_client::audit::AuditLog;
use adtp_core::{
    encryption::encrypt_file_bytes,
    integrity::hash_bytes,
    protocol::{AdtpPacket, ContextEnvelope, PacketHeader},
    transfer::{AdtpClient, AdtpServer},
    types::*,
};
use bytes::Bytes;
use chrono::Utc;
use std::{net::SocketAddr, time::Duration};
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_full_send_receive_cycle() {
    let addr = available_local_addr();
    let server = AdtpServer::bind(addr).await.unwrap();
    let server_task = tokio::spawn(async move {
        if let Ok(Some(conn)) = server.accept().await {
            let packet = conn.recv_packet().await.unwrap();
            assert_eq!(packet.header.magic, ADTP_MAGIC);
            assert_eq!(packet.header.version, ADTP_VERSION);
            assert!(matches!(packet.header.transfer_type, TransferType::Transfer));
            packet
        } else {
            panic!("No connection");
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = AdtpClient::new().await.unwrap();
    let conn = client.connect(addr, "localhost").await.unwrap();
    let session_key = [42u8; AES_KEY_SIZE];
    let file_data = b"Hello from ADTP test";
    let content_hash = hash_bytes(file_data);
    let (ciphertext, nonce) = encrypt_file_bytes(&session_key, file_data).unwrap();
    let file_meta = FileMeta {
        filename: "test.txt".to_string(),
        mime_type: "text/plain".to_string(),
        file_size: file_data.len() as u64,
        extension: "txt".to_string(),
        last_modified: None,
        content_hash,
    };
    let ctx = ContextEnvelope {
        file_meta,
        context: ContextBlock::default(),
    };
    let ctx_bytes = serde_json::to_vec(&ctx).unwrap();
    let packet = AdtpPacket {
        header: PacketHeader::new(
            TransferType::Transfer,
            Uuid::new_v4(),
            Uuid::new_v4(),
            [0u8; PEER_ID_SIZE],
            [0u8; PEER_ID_SIZE],
            nonce,
            content_hash,
            ctx_bytes.len() as u32,
            ciphertext.len() as u64,
        ),
        context: Bytes::from(ctx_bytes),
        payload: Bytes::from(ciphertext),
        signature: [0u8; ED25519_SIG_SIZE],
    };

    conn.send_packet(&packet).await.unwrap();
    let received = server_task.await.unwrap();
    assert_eq!(received.header.content_hash, content_hash);
}

#[test]
fn test_audit_log_chain() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test-audit.db");
    let log = AuditLog::open(&db_path).unwrap();
    let session_id = Uuid::new_v4();

    log.log_session_start(&session_id, "127.0.0.1:9001")
        .unwrap();
    log.log_transfer(&sample_record(session_id, "first.txt"))
        .unwrap();
    log.log_transfer(&sample_record(session_id, "second.txt"))
        .unwrap();
    log.log_session_end(&session_id, 2).unwrap();

    assert!(log.verify_chain().unwrap());
}

fn sample_record(session_id: Uuid, filename: &str) -> TransferRecord {
    let content_hash = hash_bytes(filename.as_bytes());
    TransferRecord {
        id: Uuid::new_v4(),
        session_id,
        sender_peer_id: [0u8; PEER_ID_SIZE],
        receiver_peer_id: [0u8; PEER_ID_SIZE],
        transfer_type: TransferType::Transfer,
        file_meta: FileMeta {
            filename: filename.to_string(),
            mime_type: "text/plain".to_string(),
            file_size: filename.len() as u64,
            extension: "txt".to_string(),
            last_modified: None,
            content_hash,
        },
        status: TransferStatus::Delivered,
        initiated_at: Utc::now(),
        completed_at: Some(Utc::now()),
        signature: hex::encode([0u8; ED25519_SIG_SIZE]),
        received_hash: Some(content_hash),
        integrity_verified: true,
    }
}

fn available_local_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}
