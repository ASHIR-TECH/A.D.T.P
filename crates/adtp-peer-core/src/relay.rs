use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::types::{RelayHop, RelayPath, PEER_ID_SIZE};
use chrono::Utc;

pub const DEFAULT_HOP_LIMIT: u8 = 4;

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub hop_limit: u8,
    pub relay_of: Vec<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            hop_limit: DEFAULT_HOP_LIMIT,
            relay_of: Vec::new(),
        }
    }
}

impl RelayConfig {
    pub fn can_relay_for(&self, peer_id: &str) -> bool {
        self.relay_of.is_empty() || self.relay_of.contains(&peer_id.to_string())
    }
}

pub fn check_loop(path: &RelayPath, peer_id: &[u8; PEER_ID_SIZE]) -> AdtpResult<()> {
    if path.hops.iter().any(|h| h.peer_id == *peer_id) {
        return Err(AdtpError::RelayLoopDetected {
            peer_id: hex::encode(peer_id),
        });
    }
    Ok(())
}

pub fn check_hop_limit(path: &RelayPath) -> AdtpResult<()> {
    if path.hops.len() >= path.hop_limit as usize {
        return Err(AdtpError::RelayHopLimitExceeded {
            max: path.hop_limit,
        });
    }
    Ok(())
}

pub fn verify_trust_for_next_hop(
    trust_store: &crate::trust::TrustStore,
    next_peer_id: &[u8; PEER_ID_SIZE],
) -> AdtpResult<()> {
    if !trust_store.is_allowed(next_peer_id) {
        return Err(AdtpError::NoRelayRoute {
            peer_id: hex::encode(next_peer_id),
        });
    }
    Ok(())
}

pub fn build_relay_path(hops: Vec<RelayHop>, hop_limit: u8) -> RelayPath {
    RelayPath { hops, hop_limit }
}

pub fn append_hop(path: &mut RelayPath, peer_id: [u8; PEER_ID_SIZE], host: String, port: u16) {
    path.hops.push(RelayHop {
        peer_id,
        host,
        port,
        timestamp_ns: Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
    });
}

pub fn create_relay_hint(
    relay_peer_id: [u8; PEER_ID_SIZE],
    relay_host: String,
    relay_port: u16,
) -> Vec<RelayHop> {
    vec![RelayHop {
        peer_id: relay_peer_id,
        host: relay_host,
        port: relay_port,
        timestamp_ns: Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
    }]
}

pub fn target_reached(path: &RelayPath, target_peer_id: &[u8; PEER_ID_SIZE]) -> bool {
    path.hops
        .last()
        .is_some_and(|h| h.peer_id == *target_peer_id)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayRequest {
    pub target_peer_id: [u8; PEER_ID_SIZE],
    pub target_host: String,
    pub target_port: u16,
    pub relay_via: Option<String>,
    pub packet: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UriParts {
    pub scheme: String,
    pub peer_id: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_adtp_uri(uri: &str) -> AdtpResult<UriParts> {
    let uri = uri
        .strip_prefix("adtp://")
        .ok_or_else(|| AdtpError::Internal {
            reason: format!("Invalid adtp URI scheme: {uri}"),
        })?;

    let (userinfo, rest) = uri.split_once('@').ok_or_else(|| AdtpError::Internal {
        reason: "Missing @ in adtp URI".to_string(),
    })?;

    let peer_id = userinfo.to_string();

    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, String::new()),
    };

    let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
        let port: u16 = p.parse().map_err(|_| AdtpError::Internal {
            reason: format!("Invalid port in adtp URI: {p}"),
        })?;
        (h.to_string(), port)
    } else {
        (host_port.to_string(), adtp_core::types::ADTP_DEFAULT_PORT)
    };

    Ok(UriParts {
        scheme: "adtp".to_string(),
        peer_id,
        host,
        port,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use adtp_core::types::RelayPath;

    fn make_path() -> RelayPath {
        RelayPath {
            hops: vec![RelayHop {
                peer_id: [1u8; PEER_ID_SIZE],
                host: "10.0.0.1".into(),
                port: 40001,
                timestamp_ns: 1000,
            }],
            hop_limit: 4,
        }
    }

    #[test]
    fn test_loop_detected() {
        let path = make_path();
        let result = check_loop(&path, &[1u8; PEER_ID_SIZE]);
        assert!(result.is_err());
        assert!(matches!(result, Err(AdtpError::RelayLoopDetected { .. })));
    }

    #[test]
    fn test_no_loop() {
        let path = make_path();
        assert!(check_loop(&path, &[2u8; PEER_ID_SIZE]).is_ok());
    }

    #[test]
    fn test_hop_limit_exceeded() {
        let mut path = make_path();
        path.hop_limit = 1;
        let result = check_hop_limit(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_hop_limit_ok() {
        let path = make_path();
        assert!(check_hop_limit(&path).is_ok());
    }

    #[test]
    fn test_append_hop() {
        let mut path = make_path();
        assert_eq!(path.hops.len(), 1);
        append_hop(&mut path, [3u8; PEER_ID_SIZE], "10.0.0.3".into(), 40003);
        assert_eq!(path.hops.len(), 2);
    }

    #[test]
    fn test_uri_parsing() {
        let uri = "adtp://abc123@10.0.0.1:40001/transfer/uuid-123";
        let parts = parse_adtp_uri(uri).unwrap();
        assert_eq!(parts.scheme, "adtp");
        assert_eq!(parts.peer_id, "abc123");
        assert_eq!(parts.host, "10.0.0.1");
        assert_eq!(parts.port, 40001);
        assert_eq!(parts.path, "/transfer/uuid-123");
    }

    #[test]
    fn test_uri_parsing_default_port() {
        let uri = "adtp://def456@10.0.0.2/transfer/uuid-456";
        let parts = parse_adtp_uri(uri).unwrap();
        assert_eq!(parts.port, 40001);
    }

    #[test]
    fn test_uri_parsing_invalid_scheme() {
        let result = parse_adtp_uri("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_target_reached() {
        let path = RelayPath {
            hops: vec![RelayHop {
                peer_id: [5u8; PEER_ID_SIZE],
                host: "10.0.0.5".into(),
                port: 40005,
                timestamp_ns: 5000,
            }],
            hop_limit: 4,
        };
        assert!(target_reached(&path, &[5u8; PEER_ID_SIZE]));
        assert!(!target_reached(&path, &[6u8; PEER_ID_SIZE]));
    }
}
