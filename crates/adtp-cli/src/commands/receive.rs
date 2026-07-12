use clap::Args;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::TransferList;

#[derive(Args)]
pub struct ReceiveArgs {
    #[arg(long)]
    pub pending: bool,

    #[arg(long)]
    pub delivered: bool,

    #[arg(long)]
    pub failed: bool,

    #[arg(long)]
    pub from: Option<String>,

    #[arg(long, default_value_t = 20)]
    pub limit: i64,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub status: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &ReceiveArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let limit_str = args.limit.to_string();
    let mut params: Vec<(&str, &str)> = vec![
        ("limit", &limit_str),
    ];

    // Determine status filter: shortcuts take priority
    let status = if args.pending {
        Some("pending")
    } else if args.delivered {
        Some("delivered")
    } else if args.failed {
        Some("failed")
    } else {
        args.status.as_deref()
    };

    if let Some(s) = status {
        params.push(("status", s));
    }

    if let Some(peer) = &args.from {
        params.push(("peer_id", peer));
    }

    if let Some(sid) = &args.session {
        params.push(("session_id", sid));
    }

    let list: TransferList = client.get_with_query("/api/v1/transfers", &params).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.transfers.is_empty() {
        println!("No transfers found for the given filters.");
    } else {
        println!("{}", display::render_transfer_list(&list.transfers));
        println!("Showing {} of {} total", list.transfers.len(), list.total);
    }

    Ok(0)
}
