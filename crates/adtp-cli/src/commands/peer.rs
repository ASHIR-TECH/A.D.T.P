use clap::{Args, Subcommand};
use colored::Colorize;
use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::display;
use crate::errors::CliError;
use crate::models::{PeerInfo, PeerList, ScanResult};

#[derive(Subcommand, Debug)]
pub enum PeerCommands {
    /// List all known peers
    List,
    /// Add and trust a new peer
    Add(PeerAddArgs),
    /// Remove a peer
    Remove(PeerRemoveArgs),
    /// Trust management
    Trust(PeerTrustArgs),
    /// Block a peer
    Block(PeerBlockArgs),
    /// Show own peer status (NAT, endpoint, etc.)
    Status,
    /// Discover peers on the local network
    Scan,
    /// Show detailed info about a peer
    Show(PeerShowArgs),
}

#[derive(Args, Debug)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub command: PeerCommands,
}

#[derive(Args, Debug)]
pub struct PeerAddArgs {
    pub peer_id: String,
    #[arg(long, short)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct PeerRemoveArgs {
    pub peer_id_or_nickname: String,
}

#[derive(Args, Debug)]
pub struct PeerTrustArgs {
    pub peer_id: String,
}

#[derive(Args, Debug)]
pub struct PeerBlockArgs {
    pub peer_id: String,
}

#[derive(Args, Debug)]
pub struct PeerShowArgs {
    pub peer_id_or_nickname: String,
}

pub async fn execute(
    cfg: &CliConfig,
    args: &PeerArgs,
) -> Result<i32, CliError> {
    match &args.command {
        PeerCommands::List => peer_list(cfg).await,
        PeerCommands::Add(a) => peer_add(cfg, a).await,
        PeerCommands::Remove(a) => peer_remove(cfg, a).await,
        PeerCommands::Trust(a) => peer_trust(cfg, a).await,
        PeerCommands::Block(a) => peer_block(cfg, a).await,
        PeerCommands::Status => peer_self_status(cfg).await,
        PeerCommands::Scan => peer_scan(cfg).await,
        PeerCommands::Show(a) => peer_show(cfg, a).await,
    }
}

async fn peer_list(cfg: &CliConfig) -> Result<i32, CliError> {
    if cfg.output_format == "json" {
        let list: PeerList = ApiClient::new(cfg)?.get("/api/v1/peers").await?;
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else {
        let list: PeerList = ApiClient::new(cfg)?.get("/api/v1/peers").await?;
        if list.peers.is_empty() {
            println!("No peers found.");
        } else {
            println!("{}", display::render_peer_list(&list.peers));
        }
    }
    Ok(0)
}

async fn peer_add(cfg: &CliConfig, args: &PeerAddArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    let body = serde_json::json!({
        "peer_id": args.peer_id,
        "nickname": args.name,
    });
    if cfg.output_format == "json" {
        let resp: serde_json::Value = client.post("/api/v1/peers", &body).await?;
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        let _: serde_json::Value = client.post("/api/v1/peers", &body).await?;
        let name = args.name.as_deref().unwrap_or(&args.peer_id);
        println!("{} Added peer {} ({})", "✔".green(), name, args.peer_id);
    }
    Ok(0)
}

async fn peer_remove(cfg: &CliConfig, args: &PeerRemoveArgs) -> Result<i32, CliError> {
    let client = ApiClient::new(cfg)?;
    let id = &args.peer_id_or_nickname;
    client.delete(&format!("/api/v1/peers/{id}")).await?;
    if cfg.output_format != "json" {
        println!("{} Removed peer {id}", "✔".green());
    } else {
        println!(r#"{{"removed": true, "peer": "{id}"}}"#);
    }
    Ok(0)
}

async fn peer_trust(cfg: &CliConfig, args: &PeerTrustArgs) -> Result<i32, CliError> {
    if cfg.output_format == "json" {
        let resp: serde_json::Value = ApiClient::new(cfg)?
            .post(&format!("/api/v1/peers/{}/trust", args.peer_id), &serde_json::json!({}))
            .await?;
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        let _: serde_json::Value = ApiClient::new(cfg)?
            .post(&format!("/api/v1/peers/{}/trust", args.peer_id), &serde_json::json!({}))
            .await?;
        println!("{} Trusted peer {}", "✔".green(), args.peer_id);
    }
    Ok(0)
}

async fn peer_block(cfg: &CliConfig, args: &PeerBlockArgs) -> Result<i32, CliError> {
    if cfg.output_format == "json" {
        let resp: serde_json::Value = ApiClient::new(cfg)?
            .post(&format!("/api/v1/peers/{}/block", args.peer_id), &serde_json::json!({}))
            .await?;
        println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    } else {
        let _: serde_json::Value = ApiClient::new(cfg)?
            .post(&format!("/api/v1/peers/{}/block", args.peer_id), &serde_json::json!({}))
            .await?;
        println!("{} Blocked peer {}", "✔".green(), args.peer_id);
    }
    Ok(0)
}

async fn peer_self_status(cfg: &CliConfig) -> Result<i32, CliError> {
    let info: PeerInfo = ApiClient::new(cfg)?.get("/api/v1/peers/self").await?;
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    } else {
        print!("{}", display::render_peer_status(&info));
    }
    Ok(0)
}

async fn peer_scan(cfg: &CliConfig) -> Result<i32, CliError> {
    if cfg.output_format != "json" {
        println!("Scanning local network...\n");
    }
    let result: ScanResult = ApiClient::new(cfg)?.post("/api/v1/peers/scan", &serde_json::json!({})).await?;
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else if result.peers.is_empty() {
        println!("No peers discovered on the local network.");
    } else {
        println!("{}", display::render_scan_results(&result.peers));
        println!("\nUse 'adtp peer add <peer-id> --name <nick>' to trust a discovered peer.");
    }
    Ok(0)
}

async fn peer_show(cfg: &CliConfig, args: &PeerShowArgs) -> Result<i32, CliError> {
    let info: PeerInfo = ApiClient::new(cfg)?
        .get(&format!("/api/v1/peers/{}", args.peer_id_or_nickname))
        .await?;
    if cfg.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    } else {
        let pid = display::purple(&info.peer_id);
        println!("Peer ID:     {pid}");
        if let Some(n) = &info.nickname {
            println!("Nickname:    {n}");
        }
        if let Some(h) = &info.host {
            println!("Host:        {h}");
        }
        if let Some(p) = &info.port {
            println!("Port:        {p}");
        }
        println!("Status:      {}", display::status_badge(&info.status));
        if let Some(t) = &info.trust_level {
            println!("Trust:       {t}");
        }
    }
    Ok(0)
}
