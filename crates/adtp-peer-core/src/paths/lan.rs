use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::ADTP_MDNS_SERVICE;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct LanDiscovery {
    daemon: Arc<ServiceDaemon>,
    discovered: Arc<Mutex<HashMap<String, String>>>, // peer_id -> addr
}

impl LanDiscovery {
    pub fn new() -> AdtpResult<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| AdtpError::MdnsDiscoveryFailed {
            reason: format!("mDNS daemon failed: {e}"),
        })?;

        let discovery = Self {
            daemon: Arc::new(daemon),
            discovered: Arc::new(Mutex::new(HashMap::new())),
        };

        let receiver = discovery.daemon.browse(ADTP_MDNS_SERVICE).map_err(|e| {
            AdtpError::MdnsDiscoveryFailed {
                reason: format!("browse failed: {e}"),
            }
        })?;

        let discovered = discovery.discovered.clone();
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let peer_id = info
                        .get_property("peer_id")
                        .map(|p| p.val_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let addr = format!("{}:{}", info.get_hostname(), info.get_port());
                    discovered.lock().await.insert(peer_id, addr);
                }
            }
        });

        Ok(discovery)
    }

    pub fn advertise(&self, peer_id: &[u8; 16], port: u16) -> AdtpResult<()> {
        let mut properties = HashMap::new();
        properties.insert("peer_id".to_string(), hex::encode(peer_id));
        properties.insert("adtp_version".to_string(), "1".to_string());

        let service_info = ServiceInfo::new(
            ADTP_MDNS_SERVICE,
            &hex::encode(peer_id),
            "adtp",
            "",
            port,
            properties,
        )
        .map_err(|e| AdtpError::MdnsDiscoveryFailed {
            reason: format!("ServiceInfo failed: {e}"),
        })?;

        self.daemon
            .register(service_info)
            .map_err(|e| AdtpError::MdnsDiscoveryFailed {
                reason: format!("register failed: {e}"),
            })?;

        Ok(())
    }

    pub async fn resolve(&self, peer_id: &str) -> Option<String> {
        self.discovered.lock().await.get(peer_id).cloned()
    }
}
