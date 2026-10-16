use colored::Colorize;
use comfy_table::{Cell, CellAlignment, Table};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::models::*;

// ── Blue Eclipse Colour Palette ──

pub fn gold(s: &str) -> colored::ColoredString {
    s.truecolor(201, 168, 76)
}

pub fn green(s: &str) -> colored::ColoredString {
    s.truecolor(63, 166, 107)
}

pub fn ember(s: &str) -> colored::ColoredString {
    s.truecolor(200, 67, 10)
}

pub fn purple(s: &str) -> colored::ColoredString {
    s.truecolor(124, 124, 191)
}

pub fn muted(s: &str) -> colored::ColoredString {
    s.truecolor(140, 140, 140)
}

// ── Badges ──

pub fn status_badge(status: &str) -> colored::ColoredString {
    match status {
        "delivered" => green(status).bold(),
        "pending" => gold(status).bold(),
        "in_progress" => purple(status).bold(),
        "failed" => ember(status).bold(),
        "cancelled" => purple(status).bold(),
        "queued" => gold(status).bold(),
        _ => status.normal(),
    }
}

pub fn status_badge_colored(status: &str) -> colored::ColoredString {
    status_badge(status)
}

pub fn integrity_badge(verified: bool) -> colored::ColoredString {
    if verified {
        "✓".to_string().green().bold()
    } else {
        "✗".to_string().red().bold()
    }
}

pub fn path_badge(path_type: &str) -> colored::ColoredString {
    match path_type {
        "Direct" => green(path_type).bold(),
        "Relay" => gold(path_type),
        "QoWT" => purple(path_type),
        "LAN" => purple(path_type).bold(),
        _ => muted(path_type),
    }
}

// ── Helpers ──

pub fn truncate_id(id: &str) -> String {
    if id.len() > 8 {
        format!("{}…", id[..8])
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

pub fn poll_interval() -> Duration {
    Duration::from_millis(500)
}

// ── Progress ──

pub fn new_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
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

// ── Transfer List ──

pub fn render_transfer_list(transfers: &[TransferRecord]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        muted("ID"),
        muted("Filename"),
        muted("Type"),
        muted("Size"),
        muted("Status"),
        muted("Completed"),
    ]);

    for t in transfers {
        let status_str = format!("{}", status_badge(&t.status));
        let verified = integrity_badge(t.integrity_verified);
        let completed = t.completed_at.as_deref().unwrap_or("—");
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

// ── Audit ──

pub fn render_audit_log(entries: &[AuditEntry]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        muted("ID"),
        muted("Filename"),
        muted("Type"),
        muted("Peer"),
        muted("Integrity"),
        muted("Completed"),
    ]);

    for e in entries {
        let verified = integrity_badge(e.integrity_verified);
        let completed = e.completed_at.as_deref().unwrap_or("—");
        table.add_row(vec![
            Cell::new(format!("#L-{}", truncate_id(&e.id))).set_alignment(CellAlignment::Left),
            Cell::new(&e.filename),
            Cell::new(&e.transfer_type),
            Cell::new(truncate_id(&e.session_id)),
            Cell::new(verified),
            Cell::new(completed),
        ]);
    }

    table.to_string()
}

pub fn render_chain_verify(result: &ChainVerifyResult) -> String {
    if result.valid {
        format!(
            "{} Chain VALID — {} entries verified",
            "✓".to_string().green().bold(),
            result.entries_checked
        )
    } else {
        format!(
            "{} Chain TAMPERED\nFirst failure at entry #L-{}\nExpected hash: ...\nGot: ...\nPossible data integrity breach — do not trust entries after #L-{}",
            "✗".to_string().red().bold(),
            result.first_failure_id.as_deref().unwrap_or("unknown"),
            result.first_failure_id.as_deref().unwrap_or("unknown"),
        )
    }
}

// ── Sessions ──

pub fn render_session_list(sessions: &[SessionInfo]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        muted("ID"),
        muted("Peer"),
        muted("Started"),
        muted("Ended"),
        muted("Transfers"),
        muted("Bytes"),
    ]);

    for s in sessions {
        let ended = s.ended_at.as_deref().unwrap_or("Active");
        let ended_cell = if s.ended_at.is_none() {
            Cell::new(green(ended))
        } else {
            Cell::new(ended)
        };
        table.add_row(vec![
            Cell::new(truncate_id(&s.id)).set_alignment(CellAlignment::Left),
            Cell::new(&s.peer_addr),
            Cell::new(&s.started_at),
            ended_cell,
            Cell::new(s.transfer_count.to_string()).set_alignment(CellAlignment::Right),
            Cell::new(format_file_size(s.bytes_transferred)).set_alignment(CellAlignment::Right),
        ]);
    }

    table.to_string()
}

// ── Peers ──

pub fn render_peer_list(peers: &[PeerInfo]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        muted("Name"),
        muted("Peer ID"),
        muted("Path"),
        muted("Since"),
        muted("Status"),
    ]);

    for p in peers {
        let pid = truncate_id(&p.peer_id);
        let path_str = p.path_type.as_deref().unwrap_or("—");
        let path_cell = format!("{}", path_badge(path_str));
        let status_str = format!("{}", status_badge(&p.status));
        let since = p.since.as_deref().unwrap_or("—");
        let name = p.nickname.as_deref().unwrap_or("—");
        table.add_row(vec![
            Cell::new(name),
            Cell::new(pid),
            Cell::new(path_cell),
            Cell::new(since),
            Cell::new(status_str),
        ]);
    }

    table.to_string()
}

pub fn render_peer_status(p: &PeerInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("Peer ID:     {}\n", purple(&p.peer_id)));
    if let Some(nat) = &p.nat_type {
        out.push_str(&format!("NAT Type:    {nat}\n"));
    }
    if let Some(endpoint) = &p.endpoint {
        out.push_str(&format!("Endpoint:    {endpoint}\n"));
    }
    let rv = if p.rendezvous_registered.unwrap_or(false) {
        format!(
            "Registered  {}  (last: {})",
            green("●"),
            p.rendezvous_last_seen.as_deref().unwrap_or("—")
        )
    } else {
        muted("Not registered").to_string()
    };
    out.push_str(&format!("Rendezvous:  {rv}\n"));
    out.push_str(&format!(
        "Connections: {} active\n",
        p.connections_active.unwrap_or(0)
    ));
    out
}

pub fn render_scan_results(peers: &[PeerInfo]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        muted("Nickname"),
        muted("Peer ID"),
        muted("Address"),
        muted("Trusted"),
    ]);

    for p in peers {
        let name = p.nickname.as_deref().unwrap_or("—");
        let pid = truncate_id(&p.peer_id);
        let addr = p.address.as_deref().unwrap_or("—");
        let trusted = if p.trusted.unwrap_or(false) {
            green("Yes")
        } else {
            muted("No")
        };
        table.add_row(vec![
            Cell::new(name),
            Cell::new(pid),
            Cell::new(addr),
            Cell::new(format!("{trusted}")),
        ]);
    }

    table.to_string()
}

// ── Status ──

pub fn render_status_overview(
    ready: &ReadinessResponse,
    queue: &QueueResponse,
    agent: Option<&AgentStatusResponse>,
) -> String {
    let api_status = if ready.bridge_ok {
        green("Connected")
    } else {
        ember("Disconnected")
    };
    let daemon_status = if ready.daemon_connected {
        green("Connected")
    } else {
        ember("Disconnected")
    };
    let rv_status = green("Registered");
    let lines = [
        "┌─ ADTP Status ──────────────────────────────────────────┐".to_string(),
        format!(
            "│  API          {}      http://127.0.0.1:40003    │",
            api_status
        ),
        format!(
            "│  Daemon       {}                           │",
            daemon_status
        ),
        format!(
            "│  Rendezvous   {}                               │",
            rv_status
        ),
    ];

    let mut result = lines.join("\n");
    if let Some(a) = agent {
        let agent_status = if a.active {
            green("Active")
        } else {
            muted("Inactive")
        };
        result.push_str(&format!(
            "\n│  Agent        {}         {}      │",
            agent_status, a.model
        ));
    }
    result.push_str(&format!(
        "\n│  Queue        {} pending  ·  {} in progress  ·  {} failed │",
        queue.pending, queue.in_progress, queue.failed_last_hour
    ));
    result.push_str("\n└────────────────────────────────────────────────────────┘");
    result
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

pub fn render_transfer_detail(status: &TransferStatus) -> String {
    let bar_width: usize = 20;
    let filled = (status.percent / 100.0 * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled).green(), "░".repeat(empty));
    format!(
        "Progress:  {}  {:.0}%  {} / {}\nElapsed:   {:.1}s",
        bar,
        status.percent,
        format_file_size(status.bytes_sent),
        format_file_size(status.total_bytes),
        status.elapsed_ms as f64 / 1000.0,
    )
}

// ── Agent ──

pub fn render_agent_status(status: &AgentStatusResponse) -> String {
    let active_str = if status.active {
        green("Active")
    } else {
        muted("Inactive")
    };
    let uptime = format_uptime(status.uptime_seconds);
    format!(
        "Agent:    {active_str}\n\
         Provider: {}\n\
         Model:    {}\n\
         Uptime:   {uptime}\n\
         Tasks:    {} completed today",
        status.provider, status.model, status.tasks_completed_today,
    )
}

pub fn render_agent_permissions(perms: &AgentPermissions) -> String {
    let mut lines = vec![format!(
        "{:<20} {:<30} {}",
        muted("PERMISSION"),
        muted("VALUE"),
        muted("LAST USED")
    )];
    for p in &perms.allowed_paths {
        lines.push(format!("{:<20} {:<30} {}", "Allowed paths", p, "—"));
    }
    for p in &perms.writable_paths {
        lines.push(format!("{:<20} {:<30} {}", "Writable paths", p, "—"));
    }
    for c in &perms.allowed_commands {
        lines.push(format!("{:<20} {:<30} {}", "Allowed commands", c, "—"));
    }
    lines.push(format!(
        "{:<20} {:<30}",
        "Hidden files", perms.hidden_files
    ));
    lines.push(format!(
        "{:<20} {:<30}",
        "Remote tasks", perms.remote_tasks
    ));
    lines.push(format!(
        "{:<20} {} MB",
        "File size limit", perms.file_size_limit_mb
    ));
    lines.join("\n")
}

// ── Config ──

pub fn render_config(cfg: &crate::config::CliConfig) -> String {
    format!(
        "api_url:             {}\n\
         default_peer:        {}\n\
         default_agent:       {}\n\
         agent_model:         {}\n\
         poll_interval_ms:    {}\n\
         connect_timeout_ms:  {}\n\
         request_timeout_ms:  {}\n\
         keepalive_ms:        {}\n\
         nat_override:        {}\n\
         rendezvous_server:   {}",
        cfg.api_url,
        cfg.default_peer.as_deref().unwrap_or("(not set)"),
        cfg.default_agent,
        "(auto)",
        cfg.poll_interval_ms,
        cfg.connect_timeout_ms,
        cfg.request_timeout_ms,
        cfg.keepalive_ms,
        cfg.nat_override.as_deref().unwrap_or("(auto)"),
        cfg.rendezvous_server,
    )
}

fn format_uptime(secs: i64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours} hr {minutes} min")
}

// ── Context ──

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
                out.push_str(&format!(
                    "  {}\n",
                    serde_json::to_string_pretty(other).unwrap_or_default()
                ));
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
