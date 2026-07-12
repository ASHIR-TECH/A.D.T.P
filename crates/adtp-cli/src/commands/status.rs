use clap::Args;
use std::io::{self, Write};
use std::time::Duration;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{ReadinessResponse, QueueResponse, AgentStatusResponse, TransferStatus};

#[derive(Args)]
pub struct StatusArgs {
    pub transfer_id: Option<String>,

    #[arg(long)]
    pub watch: bool,

    #[arg(long)]
    pub queue: bool,

    #[arg(long)]
    pub peer: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &StatusArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    if let Some(tid) = &args.transfer_id {
        let status: TransferStatus =
            client.get(&format!("/api/v1/transfers/{}/status", tid)).await?;

        if cfg.output_format == "json" {
            println!("{}", serde_json::to_string_pretty(&status).unwrap());
        } else if args.watch {
            loop {
                let status: TransferStatus =
                    match client.get(&format!("/api/v1/transfers/{}/status", tid)).await {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                let detail = display::render_transfer_detail(&status);
                let badge = display::status_badge(&status.status);
                let elapsed = status.elapsed_ms as f64 / 1000.0;
                let output = format!(
                    "Transfer:  {tid}\n\
                     Status:    {badge}\n\
                     {detail}\n\
                     Elapsed:   {elapsed:.1}s",
                );

                print!("\r{}", output.replace('\n', "\n\r"));
                io::stdout().flush().ok();

                match status.status.as_str() {
                    "delivered" | "failed" | "cancelled" => {
                        println!();
                        return Ok(if status.status == "delivered" { 0 } else { 1 });
                    }
                    _ => {}
                }

                tokio::time::sleep(display::poll_interval()).await;
            }
        } else {
            println!("Transfer:  {tid}");
            println!("Status:    {}", display::status_badge(&status.status));
            println!("{}", display::render_transfer_detail(&status));
        }

        return Ok(0);
    }

    // Overall status
    let ready: ReadinessResponse = client.get("/api/v1/health/ready").await?;
    let queue_resp: QueueResponse = client.get("/api/v1/health/queue").await?;
    let agent_resp: Result<AgentStatusResponse, _> = client.get("/api/v1/agent/status").await;
    let agent = agent_resp.ok();

    if cfg.output_format == "json" {
        let combined = serde_json::json!({
            "ready": ready,
            "queue": queue_resp,
            "agent": agent,
        });
        println!("{}", serde_json::to_string_pretty(&combined).unwrap());
    } else if args.queue {
        println!(
            "Queue: {} pending · {} in progress · {} failed (last hour)",
            queue_resp.pending, queue_resp.in_progress, queue_resp.failed_last_hour
        );
    } else {
        println!("{}", display::render_status_overview(&ready, &queue_resp, agent.as_ref()));
    }

    if args.watch {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let ready = match client.get::<ReadinessResponse>("/api/v1/health/ready").await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let queue_resp = match client.get::<QueueResponse>("/api/v1/health/queue").await {
                Ok(q) => q,
                Err(_) => continue,
            };
            let agent = client.get::<AgentStatusResponse>("/api/v1/agent/status").await.ok();

            let output = display::render_status_overview(&ready, &queue_resp, agent.as_ref());
            print!("\r{}", output.replace('\n', "\n\r"));
            io::stdout().flush().ok();
        }
    }

    Ok(0)
}
