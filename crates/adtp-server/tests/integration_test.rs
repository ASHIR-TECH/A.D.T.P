use adtp_core::{
    encryption::encrypt_file_bytes,
    integrity::hash_bytes,
    protocol::{AdtpPacket, ContextEnvelope, PacketHeader},
    types::*,
};
use adtp_server::{audit::AuditLog, dispatcher::Dispatcher, intake::FileIntake};
use bytes::Bytes;
use chrono::Utc;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_intake_and_dispatch_manifest() {
    let dir = tempdir().unwrap();
    let intake_dir = dir.path().join("intake");
    let output_dir = dir.path().join("output");
    let session_key = [0u8; AES_KEY_SIZE];
    let file_data = b"server intake payload";
    let content_hash = hash_bytes(file_data);
    let (ciphertext, nonce) = encrypt_file_bytes(&session_key, file_data).unwrap();
    let file_meta = FileMeta {
        filename: "payload.txt".to_string(),
        mime_type: "text/plain".to_string(),
        file_size: file_data.len() as u64,
        extension: "txt".to_string(),
        last_modified: None,
        content_hash,
    };
    let envelope = ContextEnvelope {
        file_meta,
        context: ContextBlock::default(),
    };
    let context_bytes = serde_json::to_vec(&envelope).unwrap();
    let packet = AdtpPacket {
        header: PacketHeader::new(
            TransferType::Send,
            Uuid::new_v4(),
            Uuid::new_v4(),
            nonce,
            content_hash,
            context_bytes.len() as u32,
            ciphertext.len() as u64,
        ),
        context: Bytes::from(context_bytes),
        payload: Bytes::from(ciphertext),
        signature: [0u8; ED25519_SIG_SIZE],
    };

    let intake = FileIntake::new(intake_dir.clone());
    let result = intake.process(&packet, &session_key).await.unwrap();
    assert_eq!(tokio::fs::read(&result.file_path).await.unwrap(), file_data);

    let dispatcher = Dispatcher::new(output_dir.clone());
    dispatcher.dispatch(&result).await.unwrap();
    assert!(output_dir.join("payload.txt.manifest.json").exists());
}

#[test]
fn test_audit_log_chain() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("server-audit.db");
    let log = AuditLog::open(&db_path).unwrap();
    let session_id = Uuid::new_v4();

    log.log_session_start(&session_id, "127.0.0.1:9001")
        .unwrap();
    log.log_transfer(&sample_record(session_id)).unwrap();
    log.log_session_end(&session_id, 1).unwrap();

    assert!(log.verify_chain().unwrap());
}

fn sample_record(session_id: Uuid) -> TransferRecord {
    let content_hash = hash_bytes(b"payload.txt");
    TransferRecord {
        id: Uuid::new_v4(),
        session_id,
        transfer_type: TransferType::Send,
        file_meta: FileMeta {
            filename: "payload.txt".to_string(),
            mime_type: "text/plain".to_string(),
            file_size: 11,
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
