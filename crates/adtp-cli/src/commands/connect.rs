use clap::Args;
use colored::Colorize;
use crate::config::CliConfig;
use crate::errors::CliError;

#[derive(Args)]
pub struct ConnectArgs {
    #[arg(long)]
    pub host: String,

    #[arg(long)]
    pub port: u16,

    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub default: bool,
}

pub async fn execute(cfg: &CliConfig, args: &ConnectArgs) -> Result<i32, CliError> {
    if args.port < 1 {
        return Err(CliError::Config {
            message: "Port must be between 1 and 65535.".into(),
        });
    }

    let agent = crate::config::AgentEntry {
        name: args.name.clone(),
        host: args.host.clone(),
        port: args.port,
        default: args.default,
    };

    // Load, modify, save
    let mut updated = cfg.clone();
    updated.agents.retain(|a| a.name != agent.name);
    updated.agents.push(agent);

    if args.default {
        for a in &mut updated.agents {
            a.default = a.name == args.name;
        }
    }

    updated.save()?;

    if cfg.output_format == "json" {
        println!(
            r#"{{"saved": true, "name": "{}", "host": "{}", "port": {}, "default": {}}}"#,
            args.name, args.host, args.port, args.default
        );
    } else {
        println!(
            "{} Agent '{}' saved — {}:{} — Default: {}",
            "✔".green(),
            args.name,
            args.host,
            args.port,
            if args.default { "yes" } else { "no" }
        );
    }

    Ok(0)
}
