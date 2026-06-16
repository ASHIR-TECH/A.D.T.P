use clap::Args;
use colored::Colorize;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{TransferSendResponse, TransferStatus};
use std::time::Duration;

#[derive(Args)]
pub struct SendArgs {
    pub file_path: String,

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

    let body = serde_json::json!({
        "file_path": args.file_path,
        "agent_name": args.agent,
        "priority": args.priority,
        "message": args.message,
    });

    let resp: TransferSendResponse = client.post("/api/v1/transfers/send", &body).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        println!(
            "Transfer queued — ID: {} — {}",
            resp.transfer_id,
            display::status_badge(&resp.status)
        );
    }

    if args.wait {
        let pb = display::new_progress_bar(100);
        pb.set_message("Waiting for delivery...");

        loop {
            tokio::time::sleep(Duration::from_millis(cfg.poll_interval_ms)).await;

            let status: TransferStatus =
                match client.get(&format!("/api/v1/transfers/{}/status", resp.transfer_id)).await
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
                        "{} Delivered — {}",
                        "✔".green(),
                        resp.filename
                    ));
                    return Ok(0);
                }
                "failed" => {
                    pb.finish_with_message(format!(
                        "{} Failed — {}",
                        "✘".red(),
                        resp.filename
                    ));
                    return Ok(1);
                }
                _ => {}
            }
        }
    }

    Ok(0)
}
