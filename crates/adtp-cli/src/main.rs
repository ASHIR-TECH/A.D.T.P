use clap::Parser;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_env("ADTP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let raw_args: Vec<String> = std::env::args().collect();

    // If no arguments at all, enter REPL mode directly
    if raw_args.len() <= 1 {
        let cfg = match adtp_cli::config::CliConfig::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {}", "Error".red(), e);
                std::process::exit(1);
            }
        };
        let code = adtp_cli::repl::run(&cfg).await.unwrap_or(0);
        std::process::exit(code);
    }

    // Try alias resolution (must happen before clap parsing)
    let aliased = adtp_cli::resolve_aliases(&raw_args);
    let parse_args: &[String] = aliased.as_deref().unwrap_or(&raw_args);

    // Parse CLI
    let cli = match adtp_cli::Cli::try_parse_from(parse_args) {
        Ok(c) => c,
        Err(e) => {
            // If parsing failed and we already tried aliases, give up
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // Apply --format override
    if let Some(ref fmt) = cli.format {
        std::env::set_var("ADTP_OUTPUT_FORMAT", fmt);
    }

    // Load config
    let cfg = match adtp_cli::config::CliConfig::load() {
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

    // Dispatch
    let result = match cli.command {
        Some(cmd) => dispatch(&cfg, &cmd).await,
        None => {
            // No subcommand given — enter REPL
            adtp_cli::repl::run(&cfg).await
        }
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            let code = adtp_cli::errors::exit_code(&e);
            eprintln!("{}: {}", "Error".red(), e);
            if let Some(hint) = adtp_cli::errors::hint(&e) {
                eprintln!("{}: {}", "Hint".yellow(), hint);
            }
            std::process::exit(code);
        }
    }
}

async fn dispatch(
    cfg: &adtp_cli::config::CliConfig,
    command: &adtp_cli::Commands,
) -> Result<i32, adtp_cli::errors::CliError> {
    match command {
        adtp_cli::Commands::Send(args) => adtp_cli::commands::send::execute(cfg, args).await,
        adtp_cli::Commands::Receive(args) => adtp_cli::commands::receive::execute(cfg, args).await,
        adtp_cli::Commands::Watch(args) => adtp_cli::commands::watch::execute(cfg, args).await,
        adtp_cli::Commands::Unwatch(args) => adtp_cli::commands::unwatch::execute(cfg, args).await,
        adtp_cli::Commands::Status(args) => adtp_cli::commands::status::execute(cfg, args).await,
        adtp_cli::Commands::History(args) => adtp_cli::commands::history::execute(cfg, args).await,
        adtp_cli::Commands::Verify(args) => adtp_cli::commands::verify::execute(cfg, args).await,
        adtp_cli::Commands::Connect(args) => adtp_cli::commands::connect::execute(cfg, args).await,
        adtp_cli::Commands::Context(args) => adtp_cli::commands::context::execute(cfg, args).await,
        adtp_cli::Commands::Export(args) => adtp_cli::commands::export::execute(cfg, args).await,
        adtp_cli::Commands::Session(args) => adtp_cli::commands::session::execute(cfg, args).await,
        adtp_cli::Commands::Peer(args) => adtp_cli::commands::peer::execute(cfg, args).await,
        adtp_cli::Commands::Agent(args) => adtp_cli::commands::agent::execute(cfg, args).await,
        adtp_cli::Commands::Config(args) => adtp_cli::commands::config::execute(cfg, args).await,
        adtp_cli::Commands::Audit(args) => adtp_cli::commands::audit::execute(cfg, args).await,
        adtp_cli::Commands::Ask(args) => adtp_cli::commands::agent::execute(cfg, args).await,
        adtp_cli::Commands::Desktop(args) => adtp_cli::commands::desktop::execute(cfg, args).await,
    }
}
