use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "adtp-peer",
    about = "ADTP peer daemon — symmetric transfer agent"
)]
pub struct CliArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub listen_host: String,

    #[arg(long, default_value_t = 40001)]
    pub listen_port: u16,

    #[arg(long)]
    pub intake_dir: Option<PathBuf>,

    #[arg(long)]
    pub headless: bool,

    #[arg(long, default_value_t = 40002)]
    pub agent_card_port: u16,
}
