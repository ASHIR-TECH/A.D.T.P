use clap::Args;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{ReadinessResponse, QueueResponse};

#[derive(Args)]
pub struct StatusArgs {
    pub transfer_id: Option<String>,

    #[arg(long)]
    pub watch: bool,
}

pub async fn execute(cfg: &CliConfig, args: &StatusArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    if let Some(tid) = &args.transfer_id {
        let status: crate::models::TransferStatus =
            client.get(&format!("/api/v1/transfers/{}/status", tid)).await?;

        if cfg.output_format == "json" {
            println!("{}", serde_json::to_string_pretty(&status).unwrap());
        } else {
            println!("{}", display::render_status_progress(&status));
        }

        if args.watch {
            loop {
                tokio::time::sleep(display::poll_interval()).await;
                let status: crate::models::TransferStatus =
                    match client.get(&format!("/api/v1/transfers/{}/status", tid)).await {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                if cfg.output_format != "json" {
                    print!("\r{}", display::render_status_progress(&status));
                }

                match status.status.as_str() {
                    "delivered" | "failed" | "cancelled" => {
                        println!();
                        return Ok(if status.status == "delivered" { 0 } else { 1 });
                    }
                    _ => {}
                }
            }
        }

        return Ok(0);
    }

    let ready: ReadinessResponse = client.get("/api/v1/health/ready").await?;
    let queue: QueueResponse = client.get("/api/v1/health/queue").await?;

    if cfg.output_format == "json" {
        let combined = serde_json::json!({
            "ready": ready,
            "queue": queue,
        });
        println!("{}", serde_json::to_string_pretty(&combined).unwrap());
    } else {
        use comfy_table::{Table, Cell};

        let mut table = Table::new();
        table.set_header(vec!["Metric", "Value"]);
        table.add_row(vec![
            Cell::new("Daemon Connected"),
            Cell::new(if ready.daemon_connected { "Yes" } else { "No" }),
        ]);
        table.add_row(vec![Cell::new("Queue Pending"), Cell::new(queue.pending.to_string())]);
        table.add_row(vec![Cell::new("Queue In Progress"), Cell::new(queue.in_progress.to_string())]);
        table.add_row(vec![Cell::new("Failed Last Hour"), Cell::new(queue.failed_last_hour.to_string())]);
        table.add_row(vec![Cell::new("Avg Delivery (ms)"), Cell::new(queue.avg_delivery_ms.to_string())]);
        println!("{table}");
    }

    Ok(0)
}
