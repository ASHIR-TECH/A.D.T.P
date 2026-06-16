use colored::Colorize;
use comfy_table::{Cell, CellAlignment, Table};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::models::*;

pub fn status_badge(status: &str) -> colored::ColoredString {
    match status {
        "delivered" => status.green().bold(),
        "pending" => status.yellow().bold(),
        "in_progress" => status.cyan().bold(),
        "failed" => status.red().bold(),
        "cancelled" => status.magenta().bold(),
        _ => status.normal(),
    }
}

pub fn integrity_badge(verified: bool) -> colored::ColoredString {
    if verified {
        "✓".green().bold()
    } else {
        "✗".red().bold()
    }
}

pub fn truncate_id(id: &str) -> String {
    if id.len() > 8 {
        format!("{}…", &id[..8])
    } else {
        id.to_string()
    }
}

pub fn format_file_size(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

pub fn render_transfer_list(transfers: &[TransferRecord]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Filename", "Type", "Size", "Status", "Completed",
    ]);

    for t in transfers {
        let status_str = format!("{}", status_badge(&t.status));
        let verified = integrity_badge(t.integrity_verified);
        let completed = t.completed_at.as_deref().unwrap_or("-");
        table.add_row(vec![
            Cell::new(truncate_id(&t.id)).set_alignment(CellAlignment::Left),
            Cell::new(&t.filename),
            Cell::new(&t.mime_type),
            Cell::new(format_file_size(t.file_size)).set_alignment(CellAlignment::Right),
            Cell::new(status_str),
            Cell::new(format!("{} {}", completed, verified)),
        ]);
    }

    table.to_string()
}

pub fn render_audit_log(entries: &[AuditEntry]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Filename", "Type", "Size", "Status", "Integrity", "Completed",
    ]);

    for e in entries {
        let status_str = format!("{}", status_badge(&e.status));
        let verified = integrity_badge(e.integrity_verified);
        let completed = e.completed_at.as_deref().unwrap_or("-");
        table.add_row(vec![
            Cell::new(truncate_id(&e.id)).set_alignment(CellAlignment::Left),
            Cell::new(&e.filename),
            Cell::new(&e.transfer_type),
            Cell::new(format_file_size(e.file_size)).set_alignment(CellAlignment::Right),
            Cell::new(status_str),
            Cell::new(verified),
            Cell::new(completed),
        ]);
    }

    table.to_string()
}

pub fn render_session_list(sessions: &[SessionInfo]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Peer", "Started", "Ended", "Transfers", "Bytes",
    ]);

    for s in sessions {
        let ended = s.ended_at.as_deref().unwrap_or("ACTIVE");
        table.add_row(vec![
            Cell::new(truncate_id(&s.id)).set_alignment(CellAlignment::Left),
            Cell::new(&s.peer_addr),
            Cell::new(&s.started_at),
            Cell::new(ended),
            Cell::new(s.transfer_count.to_string()).set_alignment(CellAlignment::Right),
            Cell::new(format_file_size(s.bytes_transferred)).set_alignment(CellAlignment::Right),
        ]);
    }

    table.to_string()
}

pub fn new_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb
}

pub fn new_spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb
}

pub fn render_status_progress(status: &TransferStatus) -> String {
    format!(
        "{} — {:.1}% — {}/{} — {}ms — {}",
        "Transfer".cyan(),
        status.percent,
        format_file_size(status.bytes_sent),
        format_file_size(status.total_bytes),
        status.elapsed_ms,
        status_badge(&status.status),
    )
}

pub fn render_context(ctx: &ContextBlock) -> String {
    use colored::Colorize;

    let mut out = String::new();

    if let Some(summary) = &ctx.summary {
        out.push_str(&format!("Summary: {}\n", summary.bold()));
    }

    if let Some(hint) = &ctx.agent_hint {
        out.push_str(&format!(
            "{} {}\n",
            "Agent hint:".yellow().bold(),
            hint.yellow()
        ));
    }

    if let Some(ms) = ctx.extraction_ms {
        out.push_str(&format!("Extraction time: {}ms\n", ms));
    }

    if let Some(schema) = &ctx.schema_ {
        out.push_str("\nSchema:\n");
        match schema {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    out.push_str(&format!("  {}: {}\n", k.cyan(), format_json_value(v)));
                }
            }
            other => {
                out.push_str(&format!("  {}\n", serde_json::to_string_pretty(other).unwrap_or_default()));
            }
        }
    }

    if !ctx.transfer_history.is_empty() {
        out.push_str("\nTransfer history:\n");
        for h in &ctx.transfer_history {
            out.push_str(&format!(
                "  {} — {} — {}\n",
                h.timestamp, h.mime_type, h.agent_action
            ));
        }
    }

    if out.is_empty() {
        out.push_str("No context available for this transfer.\n");
    }

    out
}

fn format_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_json_value).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_json_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        serde_json::Value::Null => "null".into(),
    }
}

pub fn poll_interval() -> Duration {
    Duration::from_millis(500)
}
