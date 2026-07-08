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

pub struct P2PModule {
    identity: PeerIdentityKey,
    endpoint: Arc<DualEndpoint>,
    rendezvous: Arc<Mutex<Option<RendezvousClient>>>,
    connections: Arc<Mutex<HashMap<String, Arc<adtp_core::transfer::AdtpConnection>>>>,
    config_dir: PathBuf,
    peer_id_hex: String,
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

        match RendezvousClient::connect("rendezvous.adtp.network", 40004).await {
            Ok(client) => {
                let peer_id = &self.identity.peer_id;
                client
                    .register(peer_id, nat.nat_type, false, true)
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
                        .register(&self.identity.peer_id, nat.nat_type, false, true)
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

    pub async fn connect_to_peer(&self, peer_id: &str) -> Result<Arc<adtp_core::transfer::AdtpConnection>, String> {
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
        handler
            .push(&path)
            .await
            .map_err(|e| format!("Failed to send file: {}", e))?;

        Ok(format!(
            "✅ Sent '{}' to peer '{}'",
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| file_path.into()),
            target_peer_id
        ))
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

        if peers.is_empty() {
            return Ok("No peers configured. Add peers in Settings.".into());
        }

        let mut lines = vec!["Known peers:".to_string()];
        for p in &peers {
            let trust = match p.trust_level {
                adtp_core::types::TrustLevel::Trusted => "trusted",
                adtp_core::types::TrustLevel::Unverified => "unverified",
                adtp_core::types::TrustLevel::Blocked => "blocked",
            };
            let nickname = p
                .nickname
                .as_deref()
                .unwrap_or(&p.peer_id[..16]);
            lines.push(format!(
                "  {} — {} ({}:{}, {})",
                nickname, p.peer_id, p.host, p.port, trust
            ));
        }
        Ok(lines.join("\n"))
    }
}

#[async_trait]
impl ToolExecutor for P2PModule {
    async fn send_file(&self, file_path: &str, peer_id: &str) -> Result<String, String> {
        self.send_file_to_peer(file_path, peer_id).await
    }

    async fn list_peers(&self) -> Result<String, String> {
        self.list_peers_text().await
    }

    async fn get_own_peer_id(&self) -> Result<String, String> {
        Ok(self.peer_id())
    }
}
