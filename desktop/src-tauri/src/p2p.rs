use adtp_core::types::{NatType, PEER_ID_SIZE};
use adtp_peer_core::agent::ToolExecutor;
use adtp_peer_core::dual_endpoint::DualEndpoint;
use adtp_peer_core::identity::PeerIdentityKey;
use adtp_peer_core::nat_detect::NatDetector;
use adtp_peer_core::push::PushHandler;
use adtp_peer_core::rendezvous_client::RendezvousClient;
use adtp_peer_core::trust::TrustStore;
use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferRecord {
    pub file_path: String,
    pub peer_id: String,
    pub status: String,
    pub timestamp: i64,
    pub details: String,
}

pub struct P2PModule {
    identity: PeerIdentityKey,
    endpoint: Arc<DualEndpoint>,
    rendezvous: Arc<Mutex<Option<RendezvousClient>>>,
    connections: Arc<Mutex<HashMap<String, Arc<adtp_core::transfer::AdtpConnection>>>>,
    config_dir: PathBuf,
    peer_id_hex: String,
    transfer_history: Arc<Mutex<Vec<TransferRecord>>>,
    nat_type: Arc<Mutex<NatType>>,
}

impl P2PModule {
    pub async fn new(config_dir: PathBuf) -> Result<Arc<Self>, String> {
        let identity_path = config_dir.join("identity.key");
        let identity = PeerIdentityKey::load_or_generate(&identity_path)
            .map_err(|e| format!("Failed to load identity: {}", e))?;

        let peer_id_hex = hex::encode(identity.peer_id);
        tracing::info!("P2P identity loaded — peer_id: {}", peer_id_hex);

        let addr: SocketAddr = format!("0.0.0.0:{}", adtp_core::types::ADTP_DEFAULT_PORT)
            .parse()
            .map_err(|e| format!("Invalid bind address: {}", e))?;

        let endpoint = Arc::new(
            DualEndpoint::bind(addr)
                .await
                .map_err(|e| format!("Failed to bind QUIC endpoint: {}", e))?,
        );

        let module = Arc::new(Self {
            identity,
            endpoint,
            rendezvous: Arc::new(Mutex::new(None)),
            connections: Arc::new(Mutex::new(HashMap::new())),
            config_dir,
            peer_id_hex,
            transfer_history: Arc::new(Mutex::new(Vec::new())),
            nat_type: Arc::new(Mutex::new(NatType::Unknown)),
        });

        module.register_with_rendezvous().await;
        module.clone().start_accept_loop();
        module.clone().start_re_registration_loop();

        Ok(module)
    }

    async fn register_with_rendezvous(&self) {
        let stun_servers = vec![
            "stun1.l.google.com:19302".to_string(),
            "stun2.l.google.com:19302".to_string(),
        ];
        let nat = NatDetector::detect(&stun_servers).unwrap_or(NatDetector {
            nat_type: NatType::Unknown,
            external_addr: None,
        });
        *self.nat_type.lock().await = nat.nat_type;

        match RendezvousClient::connect("rendezvous.adtp.network", 40004).await {
            Ok(client) => {
                let peer_id = &self.identity.peer_id;
                client
                    .register(
                        peer_id,
                        nat.nat_type,
                        false,
                        true,
                        Some(&self.identity.signing_key),
                    )
                    .await
                    .ok();
                *self.rendezvous.lock().await = Some(client);
                tracing::info!(
                    "Registered with rendezvous server (NAT: {:?})",
                    nat.nat_type
                );
            }
            Err(e) => {
                tracing::warn!("Rendezvous server unreachable: {}", e);
            }
        }
    }

    fn start_re_registration_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(240));
            loop {
                interval.tick().await;
                let guard = self.rendezvous.lock().await;
                if let Some(ref client) = *guard {
                    let stun_servers = vec![
                        "stun1.l.google.com:19302".to_string(),
                        "stun2.l.google.com:19302".to_string(),
                    ];
                    let nat = NatDetector::detect(&stun_servers).unwrap_or(NatDetector {
                        nat_type: NatType::Unknown,
                        external_addr: None,
                    });
                    if client
                        .register(
                            &self.identity.peer_id,
                            nat.nat_type,
                            false,
                            true,
                            Some(&self.identity.signing_key),
                        )
                        .await
                        .is_ok()
                    {
                        tracing::trace!("Re-registered with rendezvous server");
                    } else {
                        tracing::warn!("Re-registration with rendezvous server failed");
                        drop(guard);
                        self.register_with_rendezvous().await;
                    }
                } else {
                    drop(guard);
                    self.register_with_rendezvous().await;
                }
            }
        });
    }

    fn start_accept_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                match self.endpoint.accept().await {
                    Ok(Some(conn)) => {
                        let peer_addr = conn.peer_addr().to_string();
                        tracing::info!("Incoming connection from {}", peer_addr);
                        let mut conns = self.connections.lock().await;
                        conns.insert(peer_addr.clone(), Arc::new(conn));
                    }
                    Ok(None) => {
                        tracing::info!("P2P accept loop ended");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("P2P accept error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    pub fn peer_id(&self) -> String {
        self.peer_id_hex.clone()
    }

    pub async fn nat_type(&self) -> NatType {
        *self.nat_type.lock().await
    }

    pub async fn connect_to_peer(
        &self,
        peer_id: &str,
    ) -> Result<Arc<adtp_core::transfer::AdtpConnection>, String> {
        {
            let conns = self.connections.lock().await;
            if let Some(conn) = conns.get(peer_id) {
                return Ok(conn.clone());
            }
        }

        let peers_path = self.config_dir.join("peers.toml");
        let trust_store = TrustStore::load(&peers_path);
        let entry = trust_store
            .get_peer(peer_id)
            .ok_or_else(|| format!("Peer '{}' not found. Add them in Settings first.", peer_id))?;

        let addr: SocketAddr = format!("{}:{}", entry.host, entry.port)
            .parse()
            .map_err(|e| format!("Invalid peer address: {}", e))?;

        let conn = self
            .endpoint
            .connect(addr, "localhost")
            .await
            .map_err(|e| format!("Failed to connect to peer: {}", e))?;
        let conn = Arc::new(conn);

        let mut conns = self.connections.lock().await;
        conns.insert(peer_id.to_string(), conn.clone());

        Ok(conn)
    }

    pub async fn send_file_to_peer(
        &self,
        file_path: &str,
        target_peer_id: &str,
    ) -> Result<String, String> {
        let conn = self.connect_to_peer(target_peer_id).await?;

        let session_id = Uuid::new_v4();
        let session_key = [0u8; adtp_core::types::AES_KEY_SIZE];

        let peer_id_bytes =
            hex::decode(target_peer_id).map_err(|e| format!("Invalid peer ID hex: {}", e))?;
        let mut target_peer_bytes = [0u8; PEER_ID_SIZE];
        if peer_id_bytes.len() != PEER_ID_SIZE {
            return Err(format!(
                "Peer ID must be {} hex bytes, got {}",
                PEER_ID_SIZE,
                peer_id_bytes.len()
            ));
        }
        target_peer_bytes.copy_from_slice(&peer_id_bytes);

        let handler = PushHandler::new(
            conn,
            session_id,
            session_key,
            self.identity.peer_id,
            target_peer_bytes,
        );

        let path = PathBuf::from(file_path);
        let result = handler.push(&path).await;

        let mut history = self.transfer_history.lock().await;
        match result {
            Ok(_) => {
                let record = TransferRecord {
                    file_path: path.to_string_lossy().to_string(),
                    peer_id: target_peer_id.to_string(),
                    status: "completed".into(),
                    timestamp: chrono::Utc::now().timestamp(),
                    details: format!("Sent to peer '{}'", target_peer_id),
                };
                history.push(record);
                Ok(format!(
                    "Sent '{}' to peer '{}'",
                    path.file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_else(|| file_path.into()),
                    target_peer_id
                ))
            }
            Err(e) => {
                let record = TransferRecord {
                    file_path: path.to_string_lossy().to_string(),
                    peer_id: target_peer_id.to_string(),
                    status: "failed".into(),
                    timestamp: chrono::Utc::now().timestamp(),
                    details: format!("Failed: {}", e),
                };
                history.push(record);
                Err(format!("Failed to send file: {}", e))
            }
        }
    }

    #[allow(dead_code)]
    pub async fn disconnect_peer(&self, peer_id: &str) -> Result<(), String> {
        let mut conns = self.connections.lock().await;
        conns.remove(peer_id);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_connected_peers(&self) -> Vec<String> {
        let conns = self.connections.lock().await;
        conns.keys().cloned().collect()
    }

    pub async fn list_peers_text(&self) -> Result<String, String> {
        let peers_path = self.config_dir.join("peers.toml");
        let trust_store = TrustStore::load(&peers_path);
        let peers = trust_store.list_peers();

        let mut lines = vec!["Known peers:".to_string()];
        for p in &peers {
            let trust = match p.trust_level {
                adtp_core::types::TrustLevel::Trusted => "trusted",
                adtp_core::types::TrustLevel::Unverified => "unverified",
                adtp_core::types::TrustLevel::Blocked => "blocked",
            };
            let nickname = p.nickname.as_deref().unwrap_or(&p.peer_id[..16]);
            lines.push(format!(
                "  {} — {} ({}:{}, {})",
                nickname, p.peer_id, p.host, p.port, trust
            ));
        }

        // Paired phones live in the gateway registry, not peers.toml.
        lines.extend(Self::paired_devices_text().await);

        if lines.len() == 1 {
            return Ok("No peers configured. Add peers in Settings.".into());
        }
        Ok(lines.join("\n"))
    }

    /// Best-effort device list from the local gateway, for the agent's
    /// `list_peers` output. Any gateway failure just means "no phones".
    async fn paired_devices_text() -> Vec<String> {
        let cfg = match adtp_cli::config::CliConfig::load() {
            Ok(cfg) => cfg,
            Err(_) => return Vec::new(),
        };
        let client = match adtp_cli::client::ApiClient::new(&cfg) {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        let list = match client.list_devices().await {
            Ok(list) => list,
            Err(_) => return Vec::new(),
        };
        if list.devices.is_empty() {
            return Vec::new();
        }

        let mut lines = vec!["Paired phones (use device_id with send_file_to_device):".to_string()];
        for d in &list.devices {
            let state = if d.online { "online" } else { "offline" };
            lines.push(format!(
                "  {} — id={} ({} {}, {})",
                d.name, d.device_id, d.platform, d.os_version, state
            ));
        }
        lines
    }
}

/// Resolve a phone target for agent tools. The model often omits `device_id`
/// for "send it to my phone": one paired phone wins outright; with several,
/// only an unambiguous online phone is chosen, otherwise the error lists ids.
async fn resolve_device_target(
    client: &adtp_cli::client::ApiClient,
    device_id: &str,
) -> Result<String, String> {
    if !device_id.trim().is_empty() {
        return Ok(device_id.trim().to_string());
    }
    let list = client.list_devices().await.map_err(|e| e.to_string())?;
    match list.devices.as_slice() {
        [] => Err("No phone is paired yet. Pair MARS in ADTP Desktop → Mobile first.".into()),
        [only] => Ok(only.device_id.clone()),
        many => {
            let online: Vec<_> = many.iter().filter(|d| d.online).collect();
            if online.len() == 1 {
                Ok(online[0].device_id.clone())
            } else {
                let ids = many
                    .iter()
                    .map(|d| format!("{} ({})", d.device_id, d.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(format!(
                    "Several phones are paired; pass device_id. Pairings: {ids}"
                ))
            }
        }
    }
}

fn gateway_client() -> Result<adtp_cli::client::ApiClient, String> {
    let cfg = adtp_cli::config::CliConfig::load().map_err(|e| e.to_string())?;
    adtp_cli::client::ApiClient::new(&cfg).map_err(|e| e.to_string())
}

async fn send_file_via_gateway(file_path: &str, device_id: &str) -> Result<String, String> {
    let client = gateway_client()?;
    let target = resolve_device_target(&client, device_id).await?;
    let queued = client
        .queue_device_file(&target, file_path)
        .await
        .map_err(|e| e.to_string())?;
    // Structured result: the mobile chat parses it to render a download tile
    // with the exact device-file id, size and MIME.
    Ok(serde_json::json!({
        "status": "queued",
        "device_file_id": queued.file.id,
        "device_id": target,
        "filename": queued.file.filename,
        "size_bytes": queued.file.file_size,
        "mime_type": queued.file.mime_type,
        "message": format!(
            "Queued {} ({} bytes) for {} — it will download it from its inbox.",
            queued.file.filename, queued.file.file_size, target
        ),
    })
    .to_string())
}

async fn send_message_via_gateway(text: &str, device_id: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("send_message_to_device: text is empty".into());
    }
    let client = gateway_client()?;
    let target = resolve_device_target(&client, device_id).await?;
    let queued = client
        .send_device_message(&target, text)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "status": "sent",
        "device_id": target,
        "message_id": queued.message.id,
        "text": text,
        "message": format!("Sent to {} — it will appear in the phone's chat.", target),
    })
    .to_string())
}

async fn list_devices_via_gateway() -> Result<String, String> {
    let client = gateway_client()?;
    let list = client.list_devices().await.map_err(|e| e.to_string())?;
    if list.devices.is_empty() {
        return Ok("No phones are paired yet. Pair MARS in ADTP Desktop → Mobile first.".into());
    }
    let mut lines = vec![format!("Paired phones ({}):", list.devices.len())];
    for d in &list.devices {
        let battery = match d.battery_percent {
            Some(percent) => format!(
                "{:.0}%{}",
                percent,
                if d.battery_charging == Some(true) {
                    " (charging)"
                } else {
                    ""
                }
            ),
            None => "battery unknown".to_string(),
        };
        lines.push(format!(
            "  {} — id={} ({} {}, {}, {}, {})",
            d.name,
            d.device_id,
            d.os_name,
            d.os_version,
            d.platform,
            battery,
            if d.online { "online" } else { "offline" }
        ));
    }
    Ok(lines.join("\n"))
}

#[async_trait]
impl ToolExecutor for P2PModule {
    async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String> {
        self.send_file_to_peer(file_path, peer_id).await
    }

    async fn list_peers(&self) -> Result<String, String> {
        self.list_peers_text().await
    }

    async fn send_file_to_device(
        &self,
        file_path: &str,
        device_id: &str,
    ) -> Result<String, String> {
        send_file_via_gateway(file_path, device_id).await
    }

    async fn send_message_to_device(&self, text: &str, device_id: &str) -> Result<String, String> {
        send_message_via_gateway(text, device_id).await
    }

    async fn list_devices(&self) -> Result<String, String> {
        list_devices_via_gateway().await
    }

    async fn get_own_peer_id(&self) -> Result<String, String> {
        Ok(self.peer_id())
    }

    async fn list_transfers(&self, status_filter: Option<&str>) -> Result<String, String> {
        let history = self.transfer_history.lock().await;
        let filtered: Vec<_> = match status_filter {
            Some(s) => history.iter().filter(|r| r.status == s).collect(),
            None => history.iter().collect(),
        };
        if filtered.is_empty() {
            return Ok("No transfers found.".into());
        }
        let mut lines = vec![format!("Recent transfers ({}):", filtered.len())];
        for r in filtered.iter().rev().take(20) {
            let time = chrono::DateTime::from_timestamp(r.timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".into());
            lines.push(format!(
                "  [{}] {} — {} — {}",
                r.status, r.file_path, r.peer_id, time
            ));
        }
        Ok(lines.join("\n"))
    }
}

/// Tool executor that uses only the local gateway REST API.
///
/// The embedded agent used to get *no* executor when P2P/QUIC failed to
/// initialize, which silently disabled device listing and chat/file delivery.
/// The gateway is independent of P2P, so the agent can still see and message
/// paired phones in that case.
pub struct GatewayToolExecutor {
    pub desktop_id: String,
}

#[async_trait]
impl ToolExecutor for GatewayToolExecutor {
    async fn send_file(&self, _file_path: &str, _peer_id: &str) -> Result<String, String> {
        Err(
            "Peer-to-peer transport is unavailable. Use send_file_to_device for paired phones."
                .into(),
        )
    }

    async fn list_peers(&self) -> Result<String, String> {
        list_devices_via_gateway().await
    }

    async fn send_file_to_device(
        &self,
        file_path: &str,
        device_id: &str,
    ) -> Result<String, String> {
        send_file_via_gateway(file_path, device_id).await
    }

    async fn send_message_to_device(&self, text: &str, device_id: &str) -> Result<String, String> {
        send_message_via_gateway(text, device_id).await
    }

    async fn list_devices(&self) -> Result<String, String> {
        list_devices_via_gateway().await
    }

    async fn get_own_peer_id(&self) -> Result<String, String> {
        Ok(self.desktop_id.clone())
    }

    async fn list_transfers(&self, _status: Option<&str>) -> Result<String, String> {
        Ok("Transfer history is shown in the ADTP Desktop dashboard.".into())
    }
}
