use clap::Args;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{SessionList, SessionInfo, TransferList};

#[derive(Args)]
pub struct SessionArgs {
    pub session_id: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &SessionArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    if let Some(sid) = &args.session_id {
        let session: SessionInfo = client.get(&format!("/api/v1/sessions/{sid}")).await?;

        if cfg.output_format == "json" {
            println!("{}", serde_json::to_string_pretty(&session).unwrap());
            let transfers: TransferList = client
                .get(&format!("/api/v1/sessions/{sid}/transfers"))
                .await?;
            println!("{}", serde_json::to_string_pretty(&transfers).unwrap());
        } else {
            println!("Session: {} ({})", sid, session.peer_addr);
            println!("Started: {} — Ended: {}", session.started_at, session.ended_at.as_deref().unwrap_or("ACTIVE"));
            println!("Transfers: {} — Bytes: {}", session.transfer_count, display::format_file_size(session.bytes_transferred));
            println!();

            let transfers: TransferList = client
                .get(&format!("/api/v1/sessions/{sid}/transfers"))
                .await?;
            println!("{}", display::render_transfer_list(&transfers.transfers));
        }
    } else {
        let list: SessionList = client.get("/api/v1/sessions").await?;

        if cfg.output_format == "json" {
            println!("{}", serde_json::to_string_pretty(&list).unwrap());
        } else if list.sessions.is_empty() {
            println!("No sessions found.");
        } else {
            println!("{}", display::render_session_list(&list.sessions));
            println!("Showing {} of {} total", list.sessions.len(), list.total);
        }
    }

    Ok(0)
}
