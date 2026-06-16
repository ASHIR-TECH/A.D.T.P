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

pub async fn execute(cfg: &mut CliConfig, args: &ConnectArgs) -> Result<i32, CliError> {
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

    cfg.agents.retain(|a| a.name != agent.name);
    cfg.agents.push(agent);

    if args.default {
        for a in &mut cfg.agents {
            a.default = a.name == args.name;
        }
    }

    cfg.save()?;

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
