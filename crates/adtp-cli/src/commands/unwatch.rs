use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::errors::CliError;
use clap::Args;
use colored::Colorize;

#[derive(Args)]
pub struct UnwatchArgs {
    pub watch_id: String,
}

pub async fn execute(cfg: &CliConfig, args: &UnwatchArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    client
        .delete(&format!("/api/v1/watchers/{}", args.watch_id))
        .await?;

    if cfg.output_format == "json" {
        println!(r#"{{"removed": true, "watch_id": "{}"}}"#, args.watch_id);
    } else {
        println!("{} Watch removed: {}", "✔".green(), args.watch_id);
    }

    Ok(0)
}
