use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{TransferSendResponse, TransferStatus};
use clap::Args;
use colored::Colorize;
use std::io::IsTerminal;
use std::time::Duration;

#[derive(Args)]
pub struct SendArgs {
    pub file_path: String,

    #[arg(long, short)]
    pub to: Option<String>,

    #[arg(long, default_value = "default")]
    pub agent: String,

    #[arg(long, default_value = "normal")]
    pub priority: String,

    #[arg(long)]
    pub message: Option<String>,

    #[arg(long)]
    pub wait: bool,
}

pub async fn execute(cfg: &CliConfig, args: &SendArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    // Determine target peer
    let target_peer = match &args.to {
        Some(p) => p.clone(),
        None => match &cfg.default_peer {
            Some(p) => p.clone(),
            None => {
                // Try to get interactive peer list
                if std::io::stdin().is_terminal() {
                    let list: crate::models::PeerList = client.get("/api/v1/peers").await?;
                    if list.peers.len() == 1 {
                        list.peers[0].peer_id.clone()
                    } else if list.peers.is_empty() {
                        return Err(CliError::Config {
                            message: "No default peer set and no peers found. Use --to <peer>."
                                .into(),
                        });
                    } else {
                        return Err(CliError::Config {
                            message: "No default peer set — use --to <peer>.".into(),
                        });
                    }
                } else {
                    return Err(CliError::Config {
                        message: "No default peer set — use --to <peer>.".into(),
                    });
                }
            }
        },
    };

    let body = serde_json::json!({
        "file_path": args.file_path,
        "target_peer": target_peer,
        "agent_name": args.agent,
        "priority": args.priority,
        "message": args.message,
    });

    let resp: TransferSendResponse = client.post("/api/v1/transfers/send", &body).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        println!(
            "{:width$} {}  →  {}   ID: {}",
            display::status_badge("queued"),
            resp.filename,
            target_peer,
            resp.transfer_id,
            width = 8,
        );
        println!("Use 'adtp status {}' to track progress.", resp.transfer_id);
    }

    if args.wait {
        let pb = display::new_progress_bar(100);
        pb.set_message(format!("Sending {} to {}...", resp.filename, target_peer));

        loop {
            tokio::time::sleep(Duration::from_millis(cfg.poll_interval_ms)).await;

            let status: TransferStatus = match client
                .get(&format!("/api/v1/transfers/{}/status", resp.transfer_id))
                .await
            {
                Ok(s) => s,
                Err(_) => continue,
            };

            pb.set_position(status.percent as u64);
            pb.set_message(format!(
                "{}/{} — {}%",
                display::format_file_size(status.bytes_sent),
                display::format_file_size(status.total_bytes),
                status.percent as u64,
            ));

            match status.status.as_str() {
                "delivered" => {
                    pb.finish_with_message(format!(
                        "{} Delivered   {}   {}   {}   {:.1}s   ID: {}",
                        "✓".to_string().green(),
                        resp.filename,
                        target_peer,
                        display::format_file_size(status.total_bytes),
                        status.elapsed_ms as f64 / 1000.0,
                        resp.transfer_id,
                    ));
                    return Ok(0);
                }
                "failed" => {
                    pb.finish_with_message(format!("{} Failed — {}", "✘".red(), resp.filename));
                    return Ok(1);
                }
                "cancelled" => {
                    pb.finish_with_message("Cancelled");
                    return Ok(130);
                }
                _ => {}
            }
        }
    }

    Ok(0)
}
