use super::TrustArgs;
use crate::config::CliConfig;
use crate::errors::CliError;

fn peers_path(_cfg: &CliConfig) -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("adtp")
        .join("peers.toml")
}

pub async fn execute(cfg: &CliConfig, args: &TrustArgs) -> Result<i32, CliError> {
    let path = peers_path(cfg);
    let mut store = adtp_peer_core::trust::TrustStore::load(&path);

    if args.list {
        let peers = store.list_peers();
        if peers.is_empty() {
            println!("No trusted peers configured.");
        } else {
            println!("Trusted peers:");
            for entry in &peers {
                println!(
                    "  {} — trust level: {:?} (host: {}, port: {})",
                    entry.peer_id, entry.trust_level, entry.host, entry.port
                );
            }
        }
        return Ok(0);
    }

    if let Some(peer_id) = &args.add {
        store.add_peer(adtp_core::types::PeerEntry {
            peer_id: peer_id.clone(),
            public_key: String::new(),
            nickname: None,
            host: String::new(),
            port: 0,
            trust_level: adtp_core::types::TrustLevel::Trusted,
        });
        store.save().map_err(|e| CliError::Config {
            message: format!("Failed to save peers config: {e}"),
        })?;
        println!("Added peer {peer_id} as trusted.");
        return Ok(0);
    }

    if let Some(peer_id) = &args.remove {
        store.remove_peer(peer_id);
        store.save().map_err(|e| CliError::Config {
            message: format!("Failed to save peers config: {e}"),
        })?;
        println!("Removed peer {peer_id}.");
        return Ok(0);
    }

    eprintln!("Usage: adtp peer trust --list | --add <peer_id> | --remove <peer_id>");
    Ok(1)
}
