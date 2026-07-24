use crate::nat_detect::NatDetector;
use crate::paths;
use crate::rendezvous_client::RendezvousClient;
use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::{NatType, PeerConfig, PEER_ID_SIZE};
use ed25519_dalek::SigningKey;
use std::net::SocketAddr;

pub enum ConnectionPath {
    DirectIPv6,
    DirectIPv4,
    Lan,
    HolePunch,
    Relay,
    QoWT,
}

impl std::fmt::Display for ConnectionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionPath::DirectIPv6 => write!(f, "direct_ipv6"),
            ConnectionPath::DirectIPv4 => write!(f, "direct_ipv4"),
            ConnectionPath::Lan => write!(f, "lan"),
            ConnectionPath::HolePunch => write!(f, "hole_punch"),
            ConnectionPath::Relay => write!(f, "relay"),
            ConnectionPath::QoWT => write!(f, "qowt"),
        }
    }
}

pub struct ConnectionCoordinator {
    pub peer_id: [u8; PEER_ID_SIZE],
    pub nat_type: NatType,
    pub external_addr: Option<String>,
    pub config: PeerConfig,
    pub rendezvous_client: Option<RendezvousClient>,
}

impl ConnectionCoordinator {
    pub async fn new(
        peer_id: [u8; PEER_ID_SIZE],
        config: PeerConfig,
        signing_key: Option<&SigningKey>,
    ) -> AdtpResult<Self> {
        let nat = if let Some(ref override_type) = config.nat_type_override {
            NatDetector {
                nat_type: *override_type,
                external_addr: None,
            }
        } else {
            NatDetector::detect(&config.stun_servers)?
        };

        let rendezvous_client = if let Some(ref server) = config.rendezvous_server {
            match RendezvousClient::connect(server, config.rendezvous_port).await {
                Ok(client) => {
                    client
                        .register(&peer_id, nat.nat_type, false, config.relay_enabled, signing_key)
                        .await
                        .ok();
                    Some(client)
                }
                Err(e) => {
                    tracing::warn!("Rendezvous server unreachable: {e}");
                    None
                }
            }
        } else {
            None
        };

        tracing::info!(
            "NAT type: {:?}, external_addr: {:?}",
            nat.nat_type,
            nat.external_addr
        );

        Ok(Self {
            peer_id,
            nat_type: nat.nat_type,
            external_addr: nat.external_addr,
            config,
            rendezvous_client,
        })
    }

    pub async fn connect(
        &self,
        target_peer_id: &[u8; PEER_ID_SIZE],
        target_addr: Option<&SocketAddr>,
    ) -> AdtpResult<ConnectionPath> {
        // Step 1: IPv6 direct
        if let Some(addr) = target_addr {
            if addr.is_ipv6() {
                tracing::debug!("Step 1: trying IPv6 direct to {addr}");
                match paths::direct::dial_direct(addr, target_peer_id).await {
                    Ok(()) => return Ok(ConnectionPath::DirectIPv6),
                    Err(_) => tracing::debug!("IPv6 direct failed"),
                }
            }

            // Step 2: IPv4 direct
            tracing::debug!("Step 2: trying IPv4 direct to {addr}");
            match paths::direct::dial_direct(addr, target_peer_id).await {
                Ok(()) => return Ok(ConnectionPath::DirectIPv4),
                Err(_) => tracing::debug!("IPv4 direct failed"),
            }
        }

        // Step 3: LAN discovery (skip for now — needs mdns runtime)
        if self.config.lan_discovery {
            tracing::debug!("Step 3: LAN discovery");
            // mdns query happens here
        }

        // Step 4: STUN hole punch
        if self.nat_type != NatType::Symmetric && self.nat_type != NatType::Cgnat {
            if let Some(ref _client) = self.rendezvous_client {
                tracing::debug!("Step 4: STUN hole punch");
                // Punch implementation uses rendezvous client + stun module
            }
        } else {
            tracing::debug!("Step 4 skipped: symmetric/CGNAT NAT");
        }

        // Step 5: Relay
        if self.config.relay_enabled {
            tracing::debug!("Step 5: relay fallback");
            match paths::relay::relay_forward(target_peer_id, &[0u8; PEER_ID_SIZE], b"").await {
                Ok(()) => return Ok(ConnectionPath::Relay),
                Err(_) => tracing::debug!("Relay failed"),
            }
        }

        // Step 6: QoWT
        if self.config.qowt_enabled {
            if let Some(ref gateway) = self.config.qowt_gateway {
                tracing::debug!("Step 6: QoWT fallback");
                match paths::qowt::QowtClient::connect(gateway, target_peer_id).await {
                    Ok(()) => return Ok(ConnectionPath::QoWT),
                    Err(_) => tracing::debug!("QoWT failed"),
                }
            }
        }

        Err(AdtpError::AllPathsExhausted {
            peer_id: hex::encode(target_peer_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adtp_core::types::PeerConfig;

    #[test]
    fn test_connection_path_display() {
        let paths = vec![
            (ConnectionPath::DirectIPv6, "direct_ipv6"),
            (ConnectionPath::DirectIPv4, "direct_ipv4"),
            (ConnectionPath::Lan, "lan"),
            (ConnectionPath::HolePunch, "hole_punch"),
            (ConnectionPath::Relay, "relay"),
            (ConnectionPath::QoWT, "qowt"),
        ];
        for (path, expected) in paths {
            assert_eq!(path.to_string(), expected);
        }
    }

    #[test]
    fn test_peer_config_defaults() {
        let config = PeerConfig::default();
        assert_eq!(config.keepalive_interval_ms, 25000);
        assert!(config.lan_discovery);
        assert!(config.relay_enabled);
        assert!(config.qowt_enabled);
        assert_eq!(config.connection_timeout_ms, 3000);
        assert_eq!(config.rendezvous_port, 40004);
        assert_eq!(config.stun_servers.len(), 2);
    }
}
