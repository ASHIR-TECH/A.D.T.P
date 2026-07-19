use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{SessionInfo, SessionList, TransferList};
use clap::{Args, Subcommand};
use colored::Colorize;

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List active and past sessions
    List,
    /// Show detail view of one session
    Show(SessionShowArgs),
    /// Close an active session
    Close(SessionCloseArgs),
    /// List transfers in a session
    Transfers(SessionTransfersArgs),
}

#[derive(Args, Debug)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub command: SessionCommands,
}

#[derive(Args, Debug)]
pub struct SessionShowArgs {
    pub session_id: String,
}

#[derive(Args, Debug)]
pub struct SessionCloseArgs {
    pub session_id: String,
}

#[derive(Args, Debug)]
pub struct SessionTransfersArgs {
    pub session_id: String,
}

pub async fn execute(cfg: &CliConfig, args: &SessionArgs) -> Result<i32, CliError> {
    match &args.command {
        SessionCommands::List => session_list(cfg).await,
        SessionCommands::Show(a) => session_show(cfg, a).await,
        SessionCommands::Close(a) => session_close(cfg, a).await,
        SessionCommands::Transfers(a) => session_transfers(cfg, a).await,
    }
}

async fn session_list(cfg: &CliConfig) -> Result<i32, CliError> {
    let list: SessionList = ApiClient::new(cfg)?.get("/api/v1/sessions").await?;
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.sessions.is_empty() {
        println!("No sessions found.");
    } else {
        println!("{}", display::render_session_list(&list.sessions));
        println!("Showing {} of {} total", list.sessions.len(), list.total);
    }
    Ok(0)
}

async fn session_show(cfg: &CliConfig, args: &SessionShowArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    let session: SessionInfo = client
        .get(&format!("/api/v1/sessions/{}", args.session_id))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&session).unwrap());
        let transfers: TransferList = client
            .get(&format!("/api/v1/sessions/{}/transfers", args.session_id))
            .await?;
        println!("{}", serde_json::to_string_pretty(&transfers).unwrap());
    } else {
        let ended = session.ended_at.as_deref().unwrap_or("Active");
        println!(
            "ID:        {} ({})",
            args.session_id,
            if session.ended_at.is_none() {
                "ACTIVE".green()
            } else {
                ended.normal()
            }
        );
        println!("Peer:      {}", session.peer_addr);
        println!("Started:   {}", session.started_at);
        println!("Ended:     {ended}");
        println!("Transfers: {}", session.transfer_count);
        println!(
            "Bytes:     {}",
            display::format_file_size(session.bytes_transferred)
        );
        println!();

        let transfers: TransferList = client
            .get(&format!("/api/v1/sessions/{}/transfers", args.session_id))
            .await?;
        println!("{}", display::render_transfer_list(&transfers.transfers));
    }
    Ok(0)
}

async fn session_close(cfg: &CliConfig, args: &SessionCloseArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    client
        .delete(&format!("/api/v1/sessions/{}", args.session_id))
        .await?;
    if cfg.output_format == "json" {
        println!(r#"{{"closed": true, "session_id": "{}"}}"#, args.session_id);
    } else {
        println!("{} Session {} closed", "✔".green(), args.session_id);
    }
    Ok(0)
}

async fn session_transfers(cfg: &CliConfig, args: &SessionTransfersArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    let list: TransferList = client
        .get(&format!("/api/v1/sessions/{}/transfers", args.session_id))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.transfers.is_empty() {
        println!("No transfers in session {}.", args.session_id);
    } else {
        println!("{}", display::render_transfer_list(&list.transfers));
        println!("Showing {} of {} total", list.transfers.len(), list.total);
    }
    Ok(0)
}
