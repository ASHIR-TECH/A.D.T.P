use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

static TRUST_STORE_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_trust_store_path(path: PathBuf) {
    let _ = TRUST_STORE_PATH.set(path);
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TrustEntry {
    fingerprint: String,
    added_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TrustStore {
    peers: HashMap<String, TrustEntry>,
}

impl TrustStore {
    fn load(path: &PathBuf) -> Self {
        if let Ok(content) = fs::read_to_string(path) {
            toml::from_str(&content).unwrap_or(Self {
                peers: HashMap::new(),
            })
        } else {
            Self {
                peers: HashMap::new(),
            }
        }
    }

    fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

fn trust_store_path() -> PathBuf {
    if let Some(p) = TRUST_STORE_PATH.get() {
        return p.clone();
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("adtp")
        .join("trusted_certs.toml")
}

fn cert_fingerprint(cert: &CertificateDer) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(cert.as_ref());
    let result = hasher.finalize();
    hex::encode(result)
}

#[derive(Debug)]
pub struct TofuVerifier {
    store: Arc<Mutex<TrustStore>>,
    store_path: PathBuf,
}

impl Default for TofuVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TofuVerifier {
    pub fn new() -> Self {
        let path = trust_store_path();
        let store = TrustStore::load(&path);
        Self {
            store: Arc::new(Mutex::new(store)),
            store_path: path,
        }
    }

    pub fn ephemeral() -> Self {
        Self {
            store: Arc::new(Mutex::new(TrustStore {
                peers: HashMap::new(),
            })),
            store_path: PathBuf::from("/dev/null"),
        }
    }

    pub fn has_trusted_cert(&self, server_name: &str) -> bool {
        let store = self.store.lock().unwrap();
        store.peers.contains_key(server_name)
    }

    pub fn trust_count(&self) -> usize {
        let store = self.store.lock().unwrap();
        store.peers.len()
    }
}

impl rustls::client::danger::ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer,
        _intermediates: &[CertificateDer],
        server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let name = server_name.to_str().to_string();
        let fingerprint = cert_fingerprint(end_entity);

        let mut store = self.store.lock().unwrap();

        if let Some(entry) = store.peers.get(&name) {
            if entry.fingerprint != fingerprint {
                tracing::error!(
                    "TLS certificate changed for '{}'! \
                     Possible MITM attack. \
                     Previous fingerprint: {}, New fingerprint: {}",
                    name,
                    entry.fingerprint,
                    fingerprint
                );
                return Err(rustls::Error::General(format!(
                    "Certificate for '{}' has changed! \
                     Previous: {}, New: {}. \
                     This may indicate a MITM attack. \
                     Remove from trusted_certs.toml to re-trust.",
                    name, entry.fingerprint, fingerprint
                )));
            }
            tracing::debug!("TLS certificate verified for '{}' (TOFU match)", name);
        } else {
            tracing::warn!(
                "First contact with '{}'. Trusting certificate (TOFU). \
                 Fingerprint: {}",
                name,
                fingerprint
            );
            store.peers.insert(
                name.clone(),
                TrustEntry {
                    fingerprint: fingerprint.clone(),
                    added_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            if let Err(e) = store.save(&self.store_path) {
                tracing::error!("Failed to save trust store: {}", e);
            }
        }

        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
        .map(|_| rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
        .map(|_| rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_consistency() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let cert_der = CertificateDer::from(cert.cert.der().to_vec());
        let fp1 = cert_fingerprint(&cert_der);
        let fp2 = cert_fingerprint(&cert_der);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn test_different_certs_different_fingerprints() {
        let cert1 = rcgen::generate_simple_self_signed(vec!["a.com".to_string()]).unwrap();
        let cert2 = rcgen::generate_simple_self_signed(vec!["b.com".to_string()]).unwrap();
        let fp1 = cert_fingerprint(&CertificateDer::from(cert1.cert.der().to_vec()));
        let fp2 = cert_fingerprint(&CertificateDer::from(cert2.cert.der().to_vec()));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_trust_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trusted.toml");

        let mut store = TrustStore {
            peers: HashMap::new(),
        };
        store.peers.insert(
            "example.com".to_string(),
            TrustEntry {
                fingerprint: "abc123".to_string(),
                added_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        store.save(&path).unwrap();

        let loaded = TrustStore::load(&path);
        assert_eq!(
            loaded.peers.get("example.com").unwrap().fingerprint,
            "abc123"
        );
    }
}
