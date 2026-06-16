use clap::Args;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::TransferList;

#[derive(Args)]
pub struct ReceiveArgs {
    #[arg(long, default_value = "all")]
    pub status: String,

    #[arg(long, default_value_t = 20)]
    pub limit: i64,

    #[arg(long)]
    pub session: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &ReceiveArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let limit_str = args.limit.to_string();
    let mut params: Vec<(&str, &str)> = vec![
        ("status", args.status.as_str()),
        ("limit", &limit_str),
    ];

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
        println!(
            "Showing {} of {} total",
            list.transfers.len(),
            list.total
        );
    }

    Ok(0)
}
