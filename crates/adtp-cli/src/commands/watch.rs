use clap::Args;
use colored::Colorize;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::errors::CliError;
use crate::models::WatchResponse;

#[derive(Args)]
pub struct WatchArgs {
    pub folder_path: String,

    #[arg(long)]
    pub pattern: Option<String>,

    #[arg(long, default_value = "default")]
    pub agent: String,

    #[arg(long)]
    pub action: Option<String>,
}

pub async fn execute(cfg: &CliConfig, args: &WatchArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let body = serde_json::json!({
        "folder_path": args.folder_path,
        "file_pattern": args.pattern,
        "trigger_action": args.action,
        "agent_name": args.agent,
    });

    let resp: WatchResponse = client.post("/api/v1/watchers", &body).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        println!(
            "Watching: {} — Watch ID: {}",
            resp.folder_path,
            resp.watch_id.cyan().bold()
        );
        if let Some(p) = &resp.file_pattern {
            println!("Files matching {} will auto-send to {}", p, resp.agent_name);
        }
    }

    Ok(0)
}
