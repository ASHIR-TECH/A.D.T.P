use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::*;
use clap::{Args, Subcommand};
use colored::Colorize;

const MAX_DISPLAY_LEN: usize = 160;

#[allow(clippy::useless_borrows_in_formatting)]
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut idx = max;
    while !s.is_char_boundary(idx) {
        idx -= 1;
    }
    format!("{}… ({} more chars)", &s[..idx], s.len() - idx)
}

#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// Send a natural language instruction to the agent
    Ask(AgentAskArgs),
    /// Show agent status
    Status,
    /// Activate the agent
    Activate,
    /// Deactivate the agent
    Deactivate,
    /// View and manage agent permissions
    Permissions(AgentPermissionsArgs),
}

#[derive(Args, Debug)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommands,
}

#[derive(Args, Debug)]
pub struct AgentAskArgs {
    pub instruction: Vec<String>,
}

#[derive(Args, Debug)]
pub struct AgentPermissionsArgs {
    #[command(subcommand)]
    pub command: Option<PermissionSubcommands>,
}

#[derive(Subcommand, Debug)]
pub enum PermissionSubcommands {
    AddPath(PermissionPathArgs),
    RemovePath(PermissionPathArgs),
    AllowCommand(PermissionCommandArgs),
}

#[derive(Args, Debug)]
pub struct PermissionPathArgs {
    pub path: String,
}

#[derive(Args, Debug)]
pub struct PermissionCommandArgs {
    pub command: String,
}

impl AgentAskArgs {
    fn instruction_text(&self) -> String {
        self.instruction.join(" ")
    }
}

pub async fn execute(cfg: &CliConfig, args: &AgentArgs) -> Result<i32, CliError> {
    match &args.command {
        AgentCommands::Ask(a) => agent_ask(cfg, a).await,
        AgentCommands::Status => agent_status(cfg).await,
        AgentCommands::Activate => agent_activate(cfg).await,
        AgentCommands::Deactivate => agent_deactivate(cfg).await,
        AgentCommands::Permissions(a) => agent_permissions(cfg, a).await,
    }
}

async fn agent_ask(cfg: &CliConfig, args: &AgentAskArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    let instruction = args.instruction_text();

    if cfg.output_format != "json" {
        println!("Agent thinking...\n");
    }

    let body = serde_json::json!({ "instruction": instruction });
    let resp: AgentMessageResponse = client.post("/api/v1/agent/message", &body).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        return Ok(0);
    }

    for tc in &resp.tool_calls {
        let icon = match tc.status.as_str() {
            "completed" => "✓".green(),
            "failed" => "✗".red(),
            _ => "⟳".to_string().truecolor(124, 124, 191),
        };
        let input = truncate(&tc.input, MAX_DISPLAY_LEN);
        let result = tc
            .result
            .as_deref()
            .map(|r| truncate(r, MAX_DISPLAY_LEN))
            .unwrap_or_default();
        println!(" {icon} {}  {}  →  {}", tc.tool, input, result);
    }

    if !resp.tool_calls.is_empty() {
        println!();
    }
    println!("{}", resp.response);
    Ok(0)
}

async fn agent_status(cfg: &CliConfig) -> Result<i32, CliError> {
    let status: AgentStatusResponse = ApiClient::new(cfg)?.get("/api/v1/agent/status").await?;
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&status).unwrap());
    } else {
        println!("{}", display::render_agent_status(&status));
    }
    Ok(0)
}

async fn agent_activate(cfg: &CliConfig) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    client
        .post::<serde_json::Value>("/api/v1/agent/activate", &serde_json::json!({}))
        .await?;
    if cfg.output_format != "json" {
        println!("{} Agent activated", "✔".green());
    }
    Ok(0)
}

async fn agent_deactivate(cfg: &CliConfig) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    client
        .post::<serde_json::Value>("/api/v1/agent/deactivate", &serde_json::json!({}))
        .await?;
    if cfg.output_format != "json" {
        println!("{} Agent deactivated", "✔".green());
    }
    Ok(0)
}

async fn agent_permissions(cfg: &CliConfig, args: &AgentPermissionsArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    match &args.command {
        Some(PermissionSubcommands::AddPath(a)) => {
            let _: serde_json::Value = client
                .post(
                    "/api/v1/agent/permissions/add-path",
                    &serde_json::json!({"path": a.path}),
                )
                .await?;
            if cfg.output_format != "json" {
                println!("{} Added path: {}", "✔".green(), a.path);
            }
        }
        Some(PermissionSubcommands::RemovePath(a)) => {
            client
                .delete(&format!("/api/v1/agent/permissions/path/{}", a.path))
                .await?;
            if cfg.output_format != "json" {
                println!("{} Removed path: {}", "✔".green(), a.path);
            }
        }
        Some(PermissionSubcommands::AllowCommand(a)) => {
            let _: serde_json::Value = client
                .post(
                    "/api/v1/agent/permissions/allow-command",
                    &serde_json::json!({"command": a.command}),
                )
                .await?;
            if cfg.output_format != "json" {
                println!("{} Allowed command: {}", "✔".green(), a.command);
            }
        }
        None => {
            let perms: AgentPermissions = client.get("/api/v1/agent/permissions").await?;
            if cfg.output_format == "json" {
                println!("{}", serde_json::to_string_pretty(&perms).unwrap());
            } else {
                println!("{}", display::render_agent_permissions(&perms));
            }
        }
    }
    Ok(0)
}
