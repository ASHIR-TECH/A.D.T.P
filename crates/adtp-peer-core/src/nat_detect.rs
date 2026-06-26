use adtp_core::types::NatType;
use adtp_core::errors::{AdtpError, AdtpResult};
use std::net::UdpSocket;
use std::time::Duration;

pub struct NatDetector {
    pub nat_type: NatType,
    pub external_addr: Option<String>,
}

impl NatDetector {
    pub fn new() -> Self {
        Self {
            nat_type: NatType::Unknown,
            external_addr: None,
        }
    }

    pub fn detect(stun_servers: &[String]) -> AdtpResult<Self> {
        for server in stun_servers {
            if let Ok(result) = Self::query_stun_server(server) {
                return Ok(result);
            }
        }
        // If all STUN servers fail, assume open
        Ok(Self {
            nat_type: NatType::Open,
            external_addr: None,
        })
    }

    fn query_stun_server(server: &str) -> AdtpResult<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| AdtpError::NatDetectionFailed {
            reason: format!("bind failed: {e}"),
        })?;
        socket.set_read_timeout(Some(Duration::from_secs(2))).ok();

        // STUN binding request (RFC 8489)
        let mut req = vec![0u8; 20];
        req[0] = 0x00; req[1] = 0x01; // Binding Request
        req[2] = 0x00; req[3] = 0x0c; // Message Length
        req[4..8].copy_from_slice(&0x2112A442u32.to_be_bytes()); // Magic Cookie
        // Transaction ID (random)
        for i in 0..12 {
            req[8 + i] = rand::random::<u8>();
        }
        // Attribute: SOFTWARE (0x8022)
        let software = b"adtp-nat-detect";
        let attr_len = software.len() as u16;
        req.extend_from_slice(&0x8022u16.to_be_bytes());
        req.extend_from_slice(&attr_len.to_be_bytes());
        req.extend_from_slice(software);

        socket.send_to(&req, server).map_err(|e| AdtpError::NatDetectionFailed {
            reason: format!("send to {server} failed: {e}"),
        })?;

        let mut buf = [0u8; 512];
        let (len, _) = socket.recv_from(&mut buf).map_err(|_| AdtpError::NatDetectionFailed {
            reason: format!("no response from {server}"),
        })?;

        if len < 20 || buf[0] != 0x01 || buf[1] != 0x01 {
            return Err(AdtpError::NatDetectionFailed {
                reason: "invalid STUN response".into(),
            });
        }

        // Parse XOR-MAPPED-ADDRESS attribute
        let mut i = 20;
        while i + 4 < len {
            let attr_type = u16::from_be_bytes([buf[i], buf[i + 1]]);
            let attr_len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
            i += 4;
            if attr_type == 0x0020 {
                // XOR-MAPPED-ADDRESS
                if attr_len >= 8 && i + 8 <= len {
                    let port = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) ^ 0x2112;
                    let ip_bytes: [u8; 4] = [
                        buf[i + 4] ^ 0x21,
                        buf[i + 5] ^ 0x12,
                        buf[i + 6] ^ 0xA4,
                        buf[i + 7] ^ 0x42,
                    ];
                    let external_addr = format!("{}.{}.{}.{}:{}", ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3], port);
                    return Ok(Self {
                        nat_type: NatType::PortRestricted, // Default assumption
                        external_addr: Some(external_addr),
                    });
                }
            }
            i += attr_len;
        }

        Err(AdtpError::NatDetectionFailed {
            reason: "no XOR-MAPPED-ADDRESS in STUN response".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_detector_default() {
        let detector = NatDetector::new();
        assert_eq!(detector.nat_type, NatType::Unknown);
        assert!(detector.external_addr.is_none());
    }
}
