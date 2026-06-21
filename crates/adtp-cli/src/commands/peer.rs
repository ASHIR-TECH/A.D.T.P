use clap::{Args, Subcommand};

mod trust;

#[derive(Subcommand, Debug)]
pub enum PeerCommands {
    Trust(TrustArgs),
}

#[derive(Args, Debug)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub command: PeerCommands,
}

#[derive(Args, Debug)]
pub struct TrustArgs {
    #[arg(long)]
    pub add: Option<String>,

    #[arg(long)]
    pub remove: Option<String>,

    #[arg(long)]
    pub list: bool,
}

pub async fn execute(
    cfg: &crate::config::CliConfig,
    args: &PeerArgs,
) -> Result<i32, crate::errors::CliError> {
    match &args.command {
        PeerCommands::Trust(trust_args) => trust::execute(cfg, trust_args).await,
    }
}
