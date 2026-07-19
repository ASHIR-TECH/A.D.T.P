use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::errors::CliError;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Args)]
pub struct ExportArgs {
    #[arg(long)]
    pub output: Option<String>,

    #[arg(long)]
    pub from: Option<String>,

    #[arg(long)]
    pub to: Option<String>,

    #[arg(long)]
    pub session: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &ExportArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(f) = &args.from {
        params.push(("from_ts", f));
    }
    if let Some(t) = &args.to {
        params.push(("to_ts", t));
    }
    if let Some(sid) = &args.session {
        params.push(("session_id", sid));
    }

    let csv_content = client
        .get_raw_with_query("/api/v1/audit/export", &params)
        .await?;

    if let Some(path_str) = &args.output {
        let path = PathBuf::from(path_str);
        std::fs::write(&path, &csv_content).map_err(|e| CliError::Io {
            message: format!("Cannot write to {}: {}", path_str, e),
        })?;

        let line_count = csv_content.lines().count().saturating_sub(1);
        let check = "✔".green();
        println!("{check} Exported {line_count} entries to {path_str}");
    } else {
        print!("{csv_content}");
    }

    Ok(0)
}
