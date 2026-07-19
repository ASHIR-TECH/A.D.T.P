use adtp_cli::client::ApiClient;
use adtp_cli::config::CliConfig;
use adtp_cli::errors::CliError;
use adtp_cli::models::{QueueResponse, ReadinessResponse, SessionInfo, TransferRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub daemon_connected: bool,
    pub queue_pending: i64,
    pub queue_in_progress: i64,
    pub ready: bool,
}

#[derive(Default)]
pub struct AppState {
    pub last_transfers: Vec<TransferRecord>,
    pub last_health: Option<HealthSnapshot>,
    pub last_sessions: Vec<SessionInfo>,
}

pub type SharedState = Arc<Mutex<AppState>>;

fn make_client(
    settings: &super::settings::AppSettings,
    token: &str,
) -> Result<ApiClient, CliError> {
    let cfg = CliConfig {
        api_url: settings.api_url.clone(),
        api_token: token.to_string(),
        ..CliConfig::default()
    };
    ApiClient::new(&cfg)
}

pub fn start_polling(app: AppHandle, state: SharedState) {
    let s1 = state.clone();
    let a1 = app.clone();
    tauri::async_runtime::spawn(async move {
        poll_transfers(a1, s1).await;
    });

    let s2 = state.clone();
    let a2 = app.clone();
    tauri::async_runtime::spawn(async move {
        poll_health(a2, s2).await;
    });

    let s3 = state;
    let a3 = app;
    tauri::async_runtime::spawn(async move {
        poll_sessions(a3, s3).await;
    });
}

async fn poll_transfers(app: AppHandle, state: SharedState) {
    let mut ticker = interval(Duration::from_millis(2000));
    loop {
        ticker.tick().await;
        let settings = super::settings::read_settings(&app);
        let Some(token) = super::settings::load_api_token(&app) else {
            continue;
        };
        let Ok(client) = make_client(&settings, &token) else {
            continue;
        };

        let entries: Vec<TransferRecord> = match client
            .get_with_query::<adtp_cli::models::TransferList>(
                "/api/v1/transfers",
                &[("limit", "100")],
            )
            .await
        {
            Ok(list) => list.transfers,
            Err(_) => continue,
        };

        let mut s = state.lock().await;
        let prev = std::mem::replace(&mut s.last_transfers, entries.clone());

        for entry in &entries {
            let was_delivered = !prev
                .iter()
                .any(|p| p.id == entry.id && p.status == "delivered");
            let just_delivered = entry.status == "delivered" && was_delivered;
            let just_failed = entry.status == "failed"
                && !prev
                    .iter()
                    .any(|p| p.id == entry.id && p.status == "failed");

            if just_delivered {
                let _ = app.emit("transfer-update", entry);
                let _ = app.emit(
                    "notify",
                    serde_json::json!({
                        "title": "Transfer Delivered",
                        "body": format!("{} delivered successfully", entry.filename)
                    }),
                );
            }
            if just_failed {
                let _ = app.emit(
                    "notify",
                    serde_json::json!({
                        "title": "Transfer Failed",
                        "body": format!("{} failed to deliver", entry.filename)
                    }),
                );
            }
        }
    }
}

async fn poll_health(app: AppHandle, state: SharedState) {
    let mut ticker = interval(Duration::from_millis(5000));
    loop {
        ticker.tick().await;
        let settings = super::settings::read_settings(&app);
        let Some(token) = super::settings::load_api_token(&app) else {
            continue;
        };
        let Ok(client) = make_client(&settings, &token) else {
            continue;
        };

        let readiness = client
            .get::<ReadinessResponse>("/api/v1/health/ready")
            .await;
        let queue = client.get::<QueueResponse>("/api/v1/health/queue").await;

        let snapshot = HealthSnapshot {
            daemon_connected: readiness
                .as_ref()
                .map(|r| r.daemon_connected)
                .unwrap_or(false),
            queue_pending: queue.as_ref().map(|q| q.pending).unwrap_or(0),
            queue_in_progress: queue.as_ref().map(|q| q.in_progress).unwrap_or(0),
            ready: readiness.as_ref().map(|r| r.ready).unwrap_or(false),
        };

        {
            let mut s = state.lock().await;
            let prev_daemon = s.last_health.as_ref().map(|h| h.daemon_connected);
            s.last_health = Some(snapshot.clone());

            if prev_daemon == Some(true) && !snapshot.daemon_connected {
                let _ = app.emit(
                    "notify",
                    serde_json::json!({
                        "title": "Connection Lost",
                        "body": "ADTP daemon disconnected from API"
                    }),
                );
            }
        }

        let _ = app.emit("health-update", &snapshot);
        if let Ok(ref q) = queue {
            let _ = app.emit("queue-update", q);
        }
    }
}

async fn poll_sessions(app: AppHandle, state: SharedState) {
    let mut ticker = interval(Duration::from_millis(10000));
    loop {
        ticker.tick().await;
        let settings = super::settings::read_settings(&app);
        let Some(token) = super::settings::load_api_token(&app) else {
            continue;
        };
        let Ok(client) = make_client(&settings, &token) else {
            continue;
        };

        let sessions = match client
            .get::<adtp_cli::models::SessionList>("/api/v1/sessions")
            .await
        {
            Ok(list) => list.sessions,
            Err(_) => continue,
        };

        {
            let mut s = state.lock().await;
            s.last_sessions = sessions.clone();
        }

        let _ = app.emit("session-update", &sessions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_snapshot_default() {
        let snapshot = HealthSnapshot {
            daemon_connected: false,
            queue_pending: 0,
            queue_in_progress: 0,
            ready: false,
        };
        assert!(!snapshot.daemon_connected);
        assert!(!snapshot.ready);
    }

    #[test]
    fn test_health_snapshot_connected() {
        let snapshot = HealthSnapshot {
            daemon_connected: true,
            queue_pending: 0,
            queue_in_progress: 0,
            ready: true,
        };
        assert!(snapshot.daemon_connected);
        assert!(snapshot.ready);
    }

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert!(state.last_transfers.is_empty());
        assert!(state.last_health.is_none());
        assert!(state.last_sessions.is_empty());
    }

    #[test]
    fn test_health_snapshot_serde() {
        let s = HealthSnapshot {
            daemon_connected: true,
            queue_pending: 3,
            queue_in_progress: 1,
            ready: true,
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["daemon_connected"], true);
        assert_eq!(json["queue_pending"], 3);
        assert_eq!(json["queue_in_progress"], 1);
    }
}
