use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::AuditLogList;
use clap::Args;

#[derive(Args)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: i64,

    #[arg(long)]
    pub status: Option<String>,

    #[arg(long)]
    pub from: Option<String>,

    #[arg(long)]
    pub to: Option<String>,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long, default_value_t = 0)]
    pub offset: i64,
}

pub async fn execute(cfg: &CliConfig, args: &HistoryArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let limit_str = args.limit.to_string();
    let offset_str = args.offset.to_string();
    let mut params: Vec<(&str, &str)> = vec![("limit", &limit_str), ("offset", &offset_str)];

    if let Some(s) = &args.status {
        params.push(("status", s));
    }
    if let Some(f) = &args.from {
        params.push(("from_ts", f));
    }
    if let Some(t) = &args.to {
        params.push(("to_ts", t));
    }
    if let Some(sid) = &args.session {
        params.push(("session_id", sid));
    }

    let log: AuditLogList = client.get_with_query("/api/v1/audit/log", &params).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&log).unwrap());
    } else if log.entries.is_empty() {
        println!("No audit entries found for the given filters.");
    } else {
        println!("{}", display::render_audit_log(&log.entries));
        println!("Showing {} of {} total", log.entries.len(), log.total);
    }

    Ok(0)
}
