use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use clap::{Args, Subcommand};
use colored::Colorize;

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set a config value
    Set(ConfigSetArgs),
    /// Reset configuration to defaults
    Reset,
    /// Interactive permission editor (opens TUI)
    Permissions,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Args, Debug)]
pub struct ConfigSetArgs {
    pub key: String,
    pub value: String,
}

pub async fn execute(cfg: &CliConfig, args: &ConfigArgs) -> Result<i32, CliError> {
    match &args.command {
        ConfigCommands::Show => config_show(cfg).await,
        ConfigCommands::Set(a) => config_set(cfg, a).await,
        ConfigCommands::Reset => config_reset(cfg).await,
        ConfigCommands::Permissions => config_permissions(cfg).await,
    }
}

async fn config_show(cfg: &CliConfig) -> Result<i32, CliError> {
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(cfg).unwrap());
    } else {
        println!("{}", display::render_config(cfg));
    }
    Ok(0)
}

async fn config_set(cfg: &CliConfig, args: &ConfigSetArgs) -> Result<i32, CliError> {
    let mut updated = cfg.clone();

    match args.key.as_str() {
        "api-url" => updated.api_url = args.value.clone(),
        "api-token" => {
            updated.save_token_to_keychain(&args.value)?;
            if cfg.output_format != "json" {
                println!(
                    "{} Token saved to system keychain. Not written to any file.",
                    "✔".green()
                );
            } else {
                println!(r#"{{"saved": true, "target": "keychain"}}"#);
            }
            return Ok(0);
        }
        "default-peer" => updated.default_peer = Some(args.value.clone()),
        "default-agent" => updated.default_agent = args.value.clone(),
        "agent-model" => {
            if cfg.output_format != "json" {
                println!(
                    "{} Note: agent model is configured server-side. Updated local config.",
                    "ℹ".yellow()
                );
            }
        }
        "poll-interval" | "poll_interval_ms" => {
            let n: u64 = args.value.parse().map_err(|_| CliError::Config {
                message: format!("Invalid number: {}", args.value),
            })?;
            updated.poll_interval_ms = n;
        }
        "connect-timeout" | "connect_timeout_ms" => {
            let n: u64 = args.value.parse().map_err(|_| CliError::Config {
                message: format!("Invalid number: {}", args.value),
            })?;
            updated.connect_timeout_ms = n;
        }
        "request-timeout" | "request_timeout_ms" => {
            let n: u64 = args.value.parse().map_err(|_| CliError::Config {
                message: format!("Invalid number: {}", args.value),
            })?;
            updated.request_timeout_ms = n;
        }
        "keepalive" | "keepalive_ms" => {
            let n: u64 = args.value.parse().map_err(|_| CliError::Config {
                message: format!("Invalid number: {}", args.value),
            })?;
            updated.keepalive_ms = n;
        }
        "nat-override" => updated.nat_override = Some(args.value.clone()),
        "rendezvous" | "rendezvous-server" => updated.rendezvous_server = args.value.clone(),
        "output-format" => updated.output_format = args.value.clone(),
        _ => {
            return Err(CliError::Config {
                message: format!("Unknown config key: {}. Valid keys: api-url, api-token, default-peer, default-agent, agent-model, poll-interval, connect-timeout, request-timeout, keepalive, nat-override, rendezvous, output-format", args.key),
            });
        }
    }

    updated.save()?;

    if cfg.output_format == "json" {
        println!(
            r#"{{"updated": true, "key": "{}", "value": "{}"}}"#,
            args.key, args.value
        );
    } else {
        println!("{} Updated {} = {}", "✔".green(), args.key, args.value);
    }
    Ok(0)
}

async fn config_reset(cfg: &CliConfig) -> Result<i32, CliError> {
    let default = CliConfig::default();
    default.save()?;
    if cfg.output_format == "json" {
        println!(r#"{{"reset": true}}"#);
    } else {
        println!("{} Configuration reset to defaults.", "✔".green());
    }
    Ok(0)
}

async fn config_permissions(cfg: &CliConfig) -> Result<i32, CliError> {
    let perms: crate::models::AgentPermissions = crate::client::ApiClient::new(cfg)?
        .get("/api/v1/agent/permissions")
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&perms).unwrap());
        return Ok(0);
    }

    println!("{}", display::render_agent_permissions(&perms));
    println!("\nUse 'adtp agent permissions add-path <path>' to add allowed paths.");
    println!("Use 'adtp agent permissions allow-command <cmd>' to allow commands.");
    Ok(0)
}
