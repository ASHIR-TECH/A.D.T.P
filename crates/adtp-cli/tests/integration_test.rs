use std::process::Command;
use std::sync::OnceLock;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

static SERVER: OnceLock<MockServer> = OnceLock::new();

#[allow(dead_code)]
fn api_url() -> String {
    let server = SERVER.get_or_init(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { MockServer::start().await })
    });
    format!("http://{}", server.address())
}

#[allow(dead_code)]
fn build_cli_args(extra_args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_adtp"));
    cmd.env("ADTP_API_URL", api_url());
    cmd.env("ADTP_API_TOKEN", "test-token-abcdef1234567890");
    cmd.env("ADTP_OUTPUT_FORMAT", "json");
    cmd.env("ADTP_DEFAULT_PEER", "test-peer");
    cmd.args(extra_args);
    cmd
}

async fn setup() -> MockServer {
    MockServer::start().await
}

fn base_envs() -> Vec<(&'static str, String)> {
    vec![
        ("ADTP_API_TOKEN", "test-token-abcdef1234567890".to_string()),
        ("ADTP_OUTPUT_FORMAT", "json".to_string()),
        ("ADTP_DEFAULT_PEER", "test-peer".to_string()),
    ]
}

#[tokio::test]
async fn test_send_success() {
    let mock = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/transfers/send"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "transfer_id": "abc-123",
            "status": "queued",
            "filename": "test.txt",
            "queued_at": "2026-06-15T00:00:00Z",
            "estimated_ms": 500
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("send")
        .arg("/tmp/test.txt")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("abc-123"));
}

#[tokio::test]
async fn test_send_file_not_found() {
    let mock = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/transfers/send"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "detail": "File not found: /nonexistent/file.txt"
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("send")
        .arg("/nonexistent/file.txt")
        .output()
        .expect("failed to execute process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("File not found") || stderr.contains("404"));
}

#[tokio::test]
async fn test_receive_empty() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/transfers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transfers": [],
            "total": 0,
            "limit": 20,
            "offset": 0
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("receive")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("transfers"));
}

#[tokio::test]
async fn test_receive_with_results() {
    let mock = setup().await;

    let records = vec![
        serde_json::json!({
            "id": "abc-123-def",
            "session_id": "sess-1",
            "transfer_type": "send",
            "filename": "report.pdf",
            "mime_type": "application/pdf",
            "file_size": 1024000,
            "status": "delivered",
            "integrity_verified": true,
            "signature": "",
            "chain_hash": "",
            "initiated_at": "2026-06-15T00:00:00Z",
            "completed_at": "2026-06-15T00:00:05Z",
        }),
        serde_json::json!({
            "id": "def-456-ghi",
            "session_id": "sess-1",
            "transfer_type": "send",
            "filename": "photo.jpg",
            "mime_type": "image/jpeg",
            "file_size": 512000,
            "status": "pending",
            "integrity_verified": false,
            "signature": "",
            "chain_hash": "",
            "initiated_at": "2026-06-15T01:00:00Z",
            "completed_at": null,
        }),
        serde_json::json!({
            "id": "ghi-789-jkl",
            "session_id": "sess-2",
            "transfer_type": "push",
            "filename": "data.csv",
            "mime_type": "text/csv",
            "file_size": 256000,
            "status": "failed",
            "integrity_verified": false,
            "signature": "",
            "chain_hash": "",
            "initiated_at": "2026-06-15T02:00:00Z",
            "completed_at": "2026-06-15T02:00:03Z",
        }),
    ];

    Mock::given(method("GET"))
        .and(path("/api/v1/transfers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transfers": records,
            "total": 3,
            "limit": 20,
            "offset": 0
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("receive")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("report.pdf"));
    assert!(stdout.contains("photo.jpg"));
    assert!(stdout.contains("data.csv"));
}

#[tokio::test]
async fn test_verify_valid_chain() {
    let mock = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/audit/verify"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": true,
            "entries_checked": 150,
            "first_failure_id": null,
            "verified_at": "2026-06-15T00:00:00Z"
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("verify")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"valid\": true"));
}

#[tokio::test]
async fn test_verify_tampered_chain() {
    let mock = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/audit/verify"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": false,
            "entries_checked": 5,
            "first_failure_id": "abc-123-def",
            "verified_at": "2026-06-15T00:00:00Z"
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("verify")
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"valid\": false"));
}

#[tokio::test]
async fn test_status_daemon_down() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/health/ready"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "ready": false,
            "daemon_connected": false,
            "audit_db_ok": false,
            "bridge_ok": false
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/health/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pending": 0,
            "in_progress": 0,
            "failed_last_hour": 0,
            "avg_delivery_ms": 0
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("status")
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        assert!(stderr.contains("503") || stderr.contains("unreachable"));
    } else {
        assert!(!stdout.is_empty());
    }
}

#[tokio::test]
async fn test_export_to_file() {
    let mock = setup().await;

    let csv_content =
        "id,session_id,filename\nabc-123,sess-1,report.pdf\ndef-456,sess-1,photo.jpg\n";

    Mock::given(method("GET"))
        .and(path("/api/v1/audit/export"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(csv_content)
                .insert_header("content-type", "text/csv"),
        )
        .mount(&mock)
        .await;

    let temp_dir = std::env::temp_dir();
    let out_path = temp_dir.join("test-adtp-export.csv");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("export")
        .arg("--output")
        .arg(out_path.to_str().unwrap())
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        eprintln!("export stderr: {}", String::from_utf8_lossy(&output.stderr));
        eprintln!("export stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    assert!(output.status.success());
    let written = std::fs::read_to_string(&out_path).expect("failed to read exported file");
    assert!(written.contains("report.pdf"));
    std::fs::remove_file(&out_path).ok();
}

#[tokio::test]
async fn test_auth_invalid_token() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/transfers"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "detail": "Unauthorized"
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .env("ADTP_API_TOKEN", "invalid-token")
        .env("ADTP_DEFAULT_PEER", "test-peer")
        .env("ADTP_OUTPUT_FORMAT", "json")
        .arg("receive")
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("401") || stderr.contains("Invalid API token"));
}

#[tokio::test]
async fn test_json_output_format() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/transfers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transfers": [],
            "total": 0,
            "limit": 20,
            "offset": 0
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("receive")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify it's valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(parsed.is_object());
}

#[tokio::test]
async fn test_connect_writes_config() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", "http://127.0.0.1:40003")
        .env("ADTP_API_TOKEN", "test-token-abcdef1234567890")
        .env("ADTP_DEFAULT_PEER", "test-peer")
        .env("ADTP_OUTPUT_FORMAT", "json")
        .env("HOME", std::env::temp_dir().to_str().unwrap())
        .arg("connect")
        .arg("--host")
        .arg("192.168.1.100")
        .arg("--port")
        .arg("40001")
        .arg("--name")
        .arg("test-agent")
        .arg("--default")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-agent"));
}

#[tokio::test]
async fn test_context_empty() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/transfers/abc-123/context"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "schema": null,
            "summary": null,
            "agent_hint": null,
            "transfer_history": [],
            "extraction_ms": null
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("context")
        .arg("abc-123")
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        eprintln!("context stderr: {}", String::from_utf8_lossy(&output.stderr));
        eprintln!("context stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("null") || stdout.contains("No context"));
}

#[tokio::test]
async fn test_session_list() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sessions": [
                {
                    "id": "sess-1",
                    "peer_addr": "127.0.0.1:40001",
                    "started_at": "2026-06-15T00:00:00Z",
                    "ended_at": null,
                    "transfer_count": 5,
                    "bytes_transferred": 1048576
                }
            ],
            "total": 1
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("session")
        .arg("list")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sess-1"));
}

#[tokio::test]
async fn test_history_with_date_filter() {
    let mock = setup().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/audit/log"))
        .and(query_param("from_ts", "2026-06-01"))
        .and(query_param("to_ts", "2026-06-15"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "entries": [
                {
                    "id": "abc-123",
                    "session_id": "sess-1",
                    "transfer_type": "send",
                    "filename": "test.txt",
                    "mime_type": "text/plain",
                    "file_size": 1000,
                    "status": "delivered",
                    "integrity_verified": true,
                    "signature": "",
                    "chain_hash": "",
                    "initiated_at": "2026-06-10T00:00:00Z",
                    "completed_at": "2026-06-10T00:00:05Z"
                }
            ],
            "total": 1
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("history")
        .arg("--from")
        .arg("2026-06-01")
        .arg("--to")
        .arg("2026-06-15")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test.txt"));
}

#[tokio::test]
async fn test_send_watch_flow() {
    let mock = setup().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/watchers"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "watch_id": "watch-123",
            "folder_path": "/tmp/watchdir",
            "file_pattern": "*.csv",
            "trigger_action": null,
            "active": true,
            "agent_name": "default"
        })))
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("watch")
        .arg("/tmp/watchdir")
        .arg("--pattern")
        .arg("*.csv")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("watch-123"));
}

#[tokio::test]
async fn test_unwatch_delete() {
    let mock = setup().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/watchers/watch-123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"removed": true})),
        )
        .mount(&mock)
        .await;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adtp"))
        .env("ADTP_API_URL", format!("http://{}", mock.address()))
        .envs(base_envs())
        .arg("unwatch")
        .arg("watch-123")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
}
