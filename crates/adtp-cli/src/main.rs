mod commands;
mod client;
mod config;
mod display;
mod errors;
mod models;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "adtp", about = "ADTP CLI — Agent Data Transfer Protocol", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Send(commands::send::SendArgs),
    Receive(commands::receive::ReceiveArgs),
    Watch(commands::watch::WatchArgs),
    Unwatch(commands::unwatch::UnwatchArgs),
    Status(commands::status::StatusArgs),
    History(commands::history::HistoryArgs),
    Verify(commands::verify::VerifyArgs),
    Connect(commands::connect::ConnectArgs),
    Context(commands::context::ContextArgs),
    Export(commands::export::ExportArgs),
    Session(commands::session::SessionArgs),
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_env("ADTP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    let mut cfg = match config::CliConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {}", "Error".red(), e);
            std::process::exit(1);
        }
    };

    if let Err(e) = cfg.validate() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }

    let result = match &cli.command {
        Commands::Send(args) => commands::send::execute(&cfg, args).await,
        Commands::Receive(args) => commands::receive::execute(&cfg, args).await,
        Commands::Watch(args) => commands::watch::execute(&cfg, args).await,
        Commands::Unwatch(args) => commands::unwatch::execute(&cfg, args).await,
        Commands::Status(args) => commands::status::execute(&cfg, args).await,
        Commands::History(args) => commands::history::execute(&cfg, args).await,
        Commands::Verify(args) => commands::verify::execute(&cfg, args).await,
        Commands::Connect(args) => commands::connect::execute(&mut cfg, args).await,
        Commands::Context(args) => commands::context::execute(&cfg, args).await,
        Commands::Export(args) => commands::export::execute(&cfg, args).await,
        Commands::Session(args) => commands::session::execute(&cfg, args).await,
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            let code = errors::exit_code(&e);
            eprintln!("{}: {}", "Error".red(), e);
            std::process::exit(code);
        }
    }
}
