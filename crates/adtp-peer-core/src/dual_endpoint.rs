use adtp_core::errors::{AdtpError, AdtpResult};
use adtp_core::transfer::AdtpConnection;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::net::UdpSocket;

pub struct DualEndpoint {
    endpoint: Endpoint,
}

impl DualEndpoint {
    pub async fn bind(addr: SocketAddr) -> AdtpResult<Self> {
        let (cert, key) = generate_self_signed_cert()?;
        let server_config = build_server_config(cert, key)?;
        let client_config = build_client_config_insecure()?;

        let socket = UdpSocket::bind(addr).map_err(|e| AdtpError::BindFailed {
            addr: addr.to_string(),
            reason: e.to_string(),
        })?;

        let mut endpoint = Endpoint::new(
            Default::default(),
            Some(server_config),
            socket,
            Arc::new(quinn::TokioRuntime),
        )
        .map_err(|e| AdtpError::BindFailed {
            addr: addr.to_string(),
            reason: e.to_string(),
        })?;
        endpoint.set_default_client_config(client_config);

        tracing::info!("Dual endpoint listening on {}", addr);

        Ok(Self { endpoint })
    }

    pub async fn accept(&self) -> AdtpResult<Option<AdtpConnection>> {
        match self.endpoint.accept().await {
            Some(incoming) => {
                let connection = incoming
                    .await
                    .map_err(|e| AdtpError::QuicConnectionFailed {
                        reason: e.to_string(),
                    })?;
                let peer = connection.remote_address().to_string();
                tracing::info!("New peer connection from {}", peer);
                Ok(Some(AdtpConnection::new(connection)))
            }
            None => Ok(None),
        }
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
        server_name: &str,
    ) -> AdtpResult<AdtpConnection> {
        let connection = self
            .endpoint
            .connect(addr, server_name)
            .map_err(|e| AdtpError::QuicConnectionFailed {
                reason: e.to_string(),
            })?
            .await
            .map_err(|e| AdtpError::QuicConnectionFailed {
                reason: e.to_string(),
            })?;

        tracing::info!("Connected to peer at {}", addr);
        Ok(AdtpConnection::new(connection))
    }

    pub fn local_addr(&self) -> AdtpResult<SocketAddr> {
        self.endpoint
            .local_addr()
            .map_err(|e| AdtpError::Internal {
                reason: format!("No local addr: {e}"),
            })
    }
}

fn generate_self_signed_cert() -> AdtpResult<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .map_err(|e| AdtpError::TlsError {
            reason: e.to_string(),
        })?;
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into();
    Ok((cert_der, key_der))
}

fn build_server_config(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> AdtpResult<ServerConfig> {
    let mut server_config =
        ServerConfig::with_single_cert(vec![cert], key).map_err(|e| AdtpError::TlsError {
            reason: e.to_string(),
        })?;
    let transport =
        Arc::get_mut(&mut server_config.transport).ok_or_else(|| AdtpError::TlsError {
            reason: "Unable to configure QUIC transport".to_string(),
        })?;
    transport.max_concurrent_bidi_streams(1000u32.into());
    Ok(server_config)
}

fn build_client_config_insecure() -> AdtpResult<ClientConfig> {
    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipCertVerification))
        .with_no_client_auth();
    let quic_crypto =
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|e| AdtpError::TlsError {
                reason: e.to_string(),
            })?;
    Ok(ClientConfig::new(Arc::new(quic_crypto)))
}

#[derive(Debug)]
struct SkipCertVerification;

impl rustls::client::danger::ServerCertVerifier for SkipCertVerification {
    fn verify_server_cert(
        &self,
        _: &CertificateDer,
        _: &[CertificateDer],
        _: &rustls::pki_types::ServerName,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}
