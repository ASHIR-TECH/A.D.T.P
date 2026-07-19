use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::ContextBlock;
use clap::Args;

#[derive(Args)]
pub struct ContextArgs {
    pub transfer_id: String,
}

pub async fn execute(cfg: &CliConfig, args: &ContextArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let ctx: ContextBlock = client
        .get(&format!("/api/v1/transfers/{}/context", args.transfer_id))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&ctx).unwrap());
    } else {
        let output = display::render_context(&ctx);
        println!("{output}");
    }

    Ok(0)
}
