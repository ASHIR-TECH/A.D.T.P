use clap::Parser;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Context, Editor, Helper};
use std::borrow::Cow::{self, Owned};
use std::collections::HashSet;
use std::io::{self, Write};

use crate::art;
use crate::client::ApiClient;
use crate::commands;
use crate::config::CliConfig;
use crate::errors::CliError;
use crate::models::{AgentStatusResponse, PeerList};

const KNOWN_COMMANDS: &[&str] = &[
    "send", "receive", "recv", "status", "st", "peer", "pl", "pa", "agent", "ask", "watch",
    "unwatch", "session", "audit", "config", "history", "verify", "export", "context", "connect",
    "help", "exit", "quit", "sf",
];

struct ReplHelper {
    filenames: FilenameCompleter,
    commands: HashSet<&'static str>,
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &line[start..pos];

        let mut candidates: Vec<Pair> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Pair {
                display: cmd.to_string(),
                replacement: cmd.to_string(),
            })
            .collect();

        if candidates.is_empty() {
            candidates = self.filenames.complete(line, pos, _ctx)?.1;
        }

        Ok((start, candidates))
    }
}

impl Hinter for ReplHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Owned(line.to_string())
    }
}

impl Validator for ReplHelper {
    fn validate(
        &self,
        _ctx: &mut ValidationContext<'_>,
    ) -> Result<ValidationResult, ReadlineError> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for ReplHelper {}

pub async fn run(cfg: &CliConfig) -> Result<i32, CliError> {
    art::print_art();
    print_status_bar(cfg).await;
    println!();
    println!("Type a command (without 'adtp') or ask the agent anything.");
    println!("'help' for commands · 'exit' or Ctrl+D to quit.\n");

    let helper = ReplHelper {
        filenames: FilenameCompleter::new(),
        commands: KNOWN_COMMANDS.iter().cloned().collect(),
    };

    let mut rl = Editor::new().expect("Failed to create REPL editor");
    rl.set_helper(Some(helper));
    rl.load_history(&history_path()).ok();

    loop {
        let readline = rl.readline("▸ ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Strip "adtp" prefix if typed out of habit from the shell
                let trimmed = trimmed.strip_prefix("adtp ").unwrap_or(trimmed);
                let _ = rl.add_history_entry(trimmed);
                rl.save_history(&history_path()).ok();

                match trimmed {
                    "exit" | "quit" => {
                        println!("Goodbye.");
                        return Ok(0);
                    }
                    "help" => {
                        print_repl_help();
                    }
                    _ => {
                        io::stdout().flush().ok();
                        if let Err(e) = execute_repl_command(cfg, trimmed).await {
                            eprintln!("Error: {e}");
                            if let Some(hint) = crate::errors::hint(&e) {
                                eprintln!("Hint: {hint}");
                            }
                        }
                        io::stdout().flush().ok();
                    }
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!();
                return Ok(0);
            }
            Err(e) => {
                eprintln!("REPL error: {e}");
                let _ = rl.save_history(&history_path());
                break;
            }
        }
    }

    Ok(0)
}

async fn print_status_bar(cfg: &CliConfig) {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let gold = crate::art::gold_style();

    let peer_id = cfg.default_peer.as_deref().unwrap_or("--");

    let (peers_str, agent_str) = match ApiClient::new(cfg) {
        Ok(client) => {
            let peers = client
                .get::<PeerList>("/api/v1/peers")
                .await
                .map(|p| p.total.to_string())
                .unwrap_or_else(|_| "--".to_string());
            let agent = client
                .get::<AgentStatusResponse>("/api/v1/agent/status")
                .await
                .map(|a| {
                    if a.active {
                        format!("{} ({})", a.provider, a.model)
                    } else {
                        "inactive".into()
                    }
                })
                .unwrap_or_else(|_| "--".to_string());
            (peers, agent)
        }
        Err(_) => ("--".to_string(), "--".to_string()),
    };

    println!(
        "  {gold}{}{reset}  {dim}|{reset}  peer: {}  {dim}|{reset}  {} peers  {dim}|{reset}  agent: {}",
        crate::art::VERSION, peer_id, peers_str, agent_str,
    );
}

fn print_repl_help() {
    println!("Commands (without 'adtp' prefix):");
    println!("  send <file>         Send a file (--to, --wait, --priority)");
    println!("  receive             List incoming transfers");
    println!("  status              Overall health and queue");
    println!("  peer <subcmd>       Peer management (list, add, remove, trust, block, status, scan, show)");
    println!(
        "  agent <subcmd>      Agent commands (ask, status, activate, deactivate, permissions)"
    );
    println!("  config <subcmd>     Configuration (show, set, reset, permissions)");
    println!("  audit <subcmd>      Audit log, verify, export, show");
    println!("  session <subcmd>    Session management (list, show, close, transfers)");
    println!("  watch <folder>      Watch a folder for auto-send");
    println!("  unwatch <id>        Remove a folder watch");
    println!("  ask <instruction>   Send natural language to agent");
    println!("  help                Show this help");
    println!("  exit / quit         Exit REPL");
    println!();
    println!(
        "Natural language: Type anything that doesn't match a command and it's sent to the agent."
    );
}

async fn execute_repl_command(cfg: &CliConfig, input: &str) -> Result<i32, CliError> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(0);
    }

    let command = parts[0];
    let args = &parts[1..];

    let known_commands: HashSet<&str> = KNOWN_COMMANDS.iter().cloned().collect();

    if !known_commands.contains(command) {
        // Not a known command — send to agent
        let agent_args = commands::agent::AgentAskArgs {
            instruction: parts.iter().map(|s| s.to_string()).collect(),
        };
        let agent_cmd = commands::agent::AgentArgs {
            command: commands::agent::AgentCommands::Ask(agent_args),
        };
        return commands::agent::execute(cfg, &agent_cmd).await;
    }

    // Build argv as adtp <command> <args>
    let argv = build_repl_argv(command, args);

    let cli = match crate::Cli::try_parse_from(&argv) {
        Ok(c) => c,
        Err(_e) => {
            // Command not found or parse error — ask agent
            let agent_args = commands::agent::AgentAskArgs {
                instruction: parts.iter().map(|s| s.to_string()).collect(),
            };
            let agent_cmd = commands::agent::AgentArgs {
                command: commands::agent::AgentCommands::Ask(agent_args),
            };
            return commands::agent::execute(cfg, &agent_cmd).await;
        }
    };

    if let Some(cmd) = cli.command {
        crate::exec_command(cfg, &cmd).await
    } else {
        Ok(0)
    }
}

fn build_repl_argv(command: &str, args: &[&str]) -> Vec<String> {
    // Handle short aliases in REPL too
    let input: Vec<String> = std::iter::once("adtp".to_string())
        .chain(std::iter::once(command.to_string()))
        .chain(args.iter().map(|s| s.to_string()))
        .collect();

    crate::resolve_aliases(&input).unwrap_or(input)
}

fn history_path() -> String {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("adtp");
    std::fs::create_dir_all(&dir).ok();
    dir.join("repl_history.txt").to_string_lossy().to_string()
}
