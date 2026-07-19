use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::errors::CliError;
use crate::models::ChainVerifyResult;
use clap::Args;
use colored::Colorize;

#[derive(Args)]
pub struct VerifyArgs {}

pub async fn execute(cfg: &CliConfig, _args: &VerifyArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let result: ChainVerifyResult = client
        .post("/api/v1/audit/verify", &serde_json::json!({}))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        return Ok(0);
    }

    if result.valid {
        println!(
            "{} Audit chain VALID — {} entries verified",
            "✔".green().bold(),
            result.entries_checked
        );
        Ok(0)
    } else {
        println!(
            "{} Audit chain TAMPERED — first failure at transfer {}",
            "✘".red().bold(),
            result.first_failure_id.as_deref().unwrap_or("unknown")
        );
        println!("{} Possible data integrity breach.", "⚠".red());
        Err(CliError::ChainTampered {
            first_failure_id: result.first_failure_id.unwrap_or_default(),
        })
    }
}
