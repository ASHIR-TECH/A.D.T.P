pub mod art;
pub mod commands;
pub mod client;
pub mod config;
pub mod display;
pub mod errors;
pub mod models;
pub mod repl;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "adtp", about = "ADTP CLI — Agent Data Transfer Protocol", version)]
pub struct Cli {
    #[arg(global = true, long, value_name = "FORMAT", help = "Output format: json, table, plain")]
    pub format: Option<String>,

    #[arg(global = true, short, long, help = "Suppress all output except errors")]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Send a file — `adtp send <file> [--to <peer>] [--wait]`
    Send(commands::send::SendArgs),
    /// List incoming transfers — `adtp receive [--pending] [--from <peer>]`
    Receive(commands::receive::ReceiveArgs),
    /// Watch a folder for auto-send — `adtp watch <folder> [--to <peer>]`
    Watch(commands::watch::WatchArgs),
    /// Remove a folder watch — `adtp unwatch <watch-id>`
    Unwatch(commands::unwatch::UnwatchArgs),
    /// Service status and transfer progress — `adtp status [<id>] [--watch]`
    Status(commands::status::StatusArgs),
    /// Show audit history — `adtp history [--limit <n>]` (alias for `audit log`)
    History(commands::history::HistoryArgs),
    /// Verify audit chain integrity — `adtp verify` (alias for `audit verify`)
    Verify(commands::verify::VerifyArgs),
    /// Register an agent connection — `adtp connect --host <h> --port <p> --name <n>`
    Connect(commands::connect::ConnectArgs),
    /// Show context for a transfer — `adtp context <id>`
    Context(commands::context::ContextArgs),
    /// Export audit log as CSV — `adtp export [--out <file>]`
    Export(commands::export::ExportArgs),
    /// Session management — `adtp session <subcommand>`
    Session(commands::session::SessionArgs),
    /// Peer management — `adtp peer <subcommand>`
    Peer(commands::peer::PeerArgs),
    /// Agent commands — `adtp agent <subcommand>`
    Agent(commands::agent::AgentArgs),
    /// Configuration — `adtp config <subcommand>`
    Config(commands::config::ConfigArgs),
    /// Audit commands — `adtp audit <subcommand>`
    Audit(commands::audit::AuditArgs),
    /// Send a natural language instruction to the agent — `adtp ask "<instruction>"`
    Ask(commands::agent::AgentArgs),
    /// Launch ADTP Desktop GUI — `adtp desktop [--background]`
    Desktop(commands::desktop::DesktopArgs),
}

pub fn resolve_aliases(args: &[String]) -> Option<Vec<String>> {
    if args.len() < 2 {
        return None;
    }
    let alias = args[1].as_str();
    let resolved = match alias {
        "sf" => {
            let mut v = vec!["adtp".to_string(), "send".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "recv" => {
            let mut v = vec!["adtp".to_string(), "receive".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "st" => {
            let mut v = vec!["adtp".to_string(), "status".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "pl" => {
            let mut v = vec!["adtp".to_string(), "peer".to_string(), "list".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "pa" => {
            let mut v = vec!["adtp".to_string(), "peer".to_string(), "add".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "al" => {
            let mut v = vec!["adtp".to_string(), "audit".to_string(), "log".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "av" => {
            let mut v = vec!["adtp".to_string(), "audit".to_string(), "verify".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        "dt" => {
            let mut v = vec!["adtp".to_string(), "desktop".to_string()];
            v.extend_from_slice(&args[2..]);
            v
        }
        _ => return None,
    };
    Some(resolved)
}

pub fn dispatch_command(
    cfg: &config::CliConfig,
    command: &Commands,
) -> Result<i32, errors::CliError> {
    let result = tokio::runtime::Runtime::new()
        .expect("Failed to create runtime")
        .block_on(async {
            match command {
                Commands::Send(args) => commands::send::execute(cfg, args).await,
                Commands::Receive(args) => commands::receive::execute(cfg, args).await,
                Commands::Watch(args) => commands::watch::execute(cfg, args).await,
                Commands::Unwatch(args) => commands::unwatch::execute(cfg, args).await,
                Commands::Status(args) => commands::status::execute(cfg, args).await,
                Commands::History(args) => commands::history::execute(cfg, args).await,
                Commands::Verify(args) => commands::verify::execute(cfg, args).await,
                Commands::Connect(args) => commands::connect::execute(cfg, args).await,
                Commands::Context(args) => commands::context::execute(cfg, args).await,
                Commands::Export(args) => commands::export::execute(cfg, args).await,
                Commands::Session(args) => commands::session::execute(cfg, args).await,
                Commands::Peer(args) => commands::peer::execute(cfg, args).await,
                Commands::Agent(args) => commands::agent::execute(cfg, args).await,
                Commands::Config(args) => commands::config::execute(cfg, args).await,
                Commands::Audit(args) => commands::audit::execute(cfg, args).await,
                Commands::Ask(args) => commands::agent::execute(cfg, args).await,
                Commands::Desktop(args) => commands::desktop::execute(cfg, args).await,
            }
        });
    result
}
