use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::*;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum AuditCommands {
    /// View audit log
    Log(AuditLogArgs),
    /// Verify audit chain integrity
    Verify,
    /// Export audit log as CSV
    Export(AuditExportArgs),
    /// Show detailed information about a transfer
    Show(AuditShowArgs),
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    #[command(subcommand)]
    pub command: AuditCommands,
}

#[derive(Args, Debug)]
pub struct AuditLogArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: i64,
    #[arg(long)]
    pub from: Option<String>,
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub peer: Option<String>,
}

#[derive(Args, Debug)]
pub struct AuditExportArgs {
    #[arg(long)]
    pub out: Option<String>,
    #[arg(long)]
    pub from: Option<String>,
    #[arg(long)]
    pub to: Option<String>,
}

#[derive(Args, Debug)]
pub struct AuditShowArgs {
    pub transfer_id: String,
}

pub async fn execute(cfg: &CliConfig, args: &AuditArgs) -> Result<i32, CliError> {
    match &args.command {
        AuditCommands::Log(a) => audit_log(cfg, a).await,
        AuditCommands::Verify => audit_verify(cfg).await,
        AuditCommands::Export(a) => audit_export(cfg, a).await,
        AuditCommands::Show(a) => audit_show(cfg, a).await,
    }
}

async fn audit_log(cfg: &CliConfig, args: &AuditLogArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let limit_str = args.limit.to_string();
    let mut params: Vec<(&str, &str)> = vec![("limit", &limit_str)];

    if let Some(s) = &args.status {
        params.push(("status", s));
    }
    if let Some(f) = &args.from {
        params.push(("from_ts", f));
    }
    if let Some(t) = &args.to {
        params.push(("to_ts", t));
    }
    if let Some(p) = &args.peer {
        params.push(("peer_id", p));
    }

    let log: AuditLogList = client.get_with_query("/api/v1/audit/log", &params).await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&log).unwrap());
    } else if log.entries.is_empty() {
        println!("No audit entries found for the given filters.");
    } else {
        println!("{}", display::render_audit_log(&log.entries));
        println!("Showing {} of {} total", log.entries.len(), log.total);
    }
    Ok(0)
}

async fn audit_verify(cfg: &CliConfig) -> Result<i32, CliError> {
    if cfg.output_format != "json" {
        println!("Verifying audit chain...\n");
    }

    let result: ChainVerifyResult = ApiClient::new(cfg)?
        .post("/api/v1/audit/verify", &serde_json::json!({}))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        return Ok(0);
    }

    if result.valid {
        println!(
            "{} Chain VALID — {} entries verified in 0.3s",
            "✓".to_string().green().bold(),
            result.entries_checked
        );
        Ok(0)
    } else {
        println!(
            "{} Chain TAMPERED\n  First failure at entry #L-{}\n  {} Possible data integrity breach — do not trust entries after #L-{}",
            "✗".to_string().red().bold(),
            result.first_failure_id.as_deref().unwrap_or("unknown"),
            "⚠".red(),
            result.first_failure_id.as_deref().unwrap_or("unknown"),
        );
        Err(CliError::ChainTampered {
            first_failure_id: result.first_failure_id.unwrap_or_default(),
        })
    }
}

async fn audit_export(cfg: &CliConfig, args: &AuditExportArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(f) = &args.from {
        params.push(("from_ts", f));
    }
    if let Some(t) = &args.to {
        params.push(("to_ts", t));
    }

    let csv_content = client
        .get_raw_with_query("/api/v1/audit/export", &params)
        .await?;

    if let Some(path_str) = &args.out {
        let path = PathBuf::from(path_str);
        std::fs::write(&path, &csv_content).map_err(|e| CliError::Io {
            message: format!("Cannot write to {}: {}", path_str, e),
        })?;

        let line_count = csv_content.lines().count().saturating_sub(1);
        if cfg.output_format == "json" {
            println!(
                r#"{{"exported": true, "entries": {}, "path": "{}"}}"#,
                line_count, path_str
            );
        } else {
            println!(
                "{} Exported {} entries to {}",
                "✓".green(),
                line_count,
                path_str
            );
        }
    } else {
        print!("{csv_content}");
    }
    Ok(0)
}

async fn audit_show(cfg: &CliConfig, args: &AuditShowArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;

    let record: TransferRecord = client
        .get(&format!("/api/v1/transfers/{}", args.transfer_id))
        .await?;

    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&record).unwrap());
    } else {
        println!("Transfer:  {}", args.transfer_id);
        println!(
            "File:      {} ({})",
            record.filename,
            display::format_file_size(record.file_size)
        );
        println!("Type:      {}", record.transfer_type);
        println!("Status:    {}", display::status_badge(&record.status));
        println!(
            "Integrity: {}",
            display::integrity_badge(record.integrity_verified)
        );
        if let Some(completed) = &record.completed_at {
            println!("Completed: {completed}");
        }
        println!("Initiated: {}", record.initiated_at);
    }
    Ok(0)
}
