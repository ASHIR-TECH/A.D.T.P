use bytes::Bytes;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::{Arc, Once};

use crate::errors::{AdtpError, AdtpResult};
use crate::protocol::AdtpPacket;

const LENGTH_PREFIX_SIZE: usize = 8;

pub struct AdtpServer {
    endpoint: Endpoint,
}

impl AdtpServer {
    pub async fn bind(addr: SocketAddr) -> AdtpResult<Self> {
        install_crypto_provider_once();
        let (cert, key) = generate_self_signed_cert()?;
        let server_config = build_server_config(cert, key)?;
        let endpoint =
            Endpoint::server(server_config, addr).map_err(|e| AdtpError::BindFailed {
                addr: addr.to_string(),
                reason: e.to_string(),
            })?;

        tracing::info!("ADTP server listening on {}", addr);

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

                tracing::info!("New ADTP connection from {}", peer);

                Ok(Some(AdtpConnection::new(connection)))
            }
            None => Ok(None),
        }
    }
}

pub struct AdtpClient {
    endpoint: Endpoint,
}

impl AdtpClient {
    pub async fn new() -> AdtpResult<Self> {
        install_crypto_provider_once();
        let client_config = build_client_config_insecure()?;
        let mut endpoint = Endpoint::client(
            "0.0.0.0:0".parse().expect("valid client bind address"),
        )
        .map_err(|e| AdtpError::QuicConnectionFailed {
            reason: e.to_string(),
        })?;

        endpoint.set_default_client_config(client_config);

        Ok(Self { endpoint })
    }

    pub async fn connect(&self, addr: SocketAddr, server_name: &str) -> AdtpResult<AdtpConnection> {
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

        tracing::info!("Connected to ADTP server at {}", addr);

        Ok(AdtpConnection::new(connection))
    }
}

pub struct AdtpConnection {
    inner: Connection,
}

impl AdtpConnection {
    pub fn new(connection: Connection) -> Self {
        Self { inner: connection }
    }

    pub async fn send_packet(&self, packet: &AdtpPacket) -> AdtpResult<()> {
        let (mut send, _recv) =
            self.inner
                .open_bi()
                .await
                .map_err(|e| AdtpError::QuicStreamError {
                    reason: e.to_string(),
                })?;
        let packet_bytes = packet.to_bytes()?;

        write_framed(&mut send, &packet_bytes).await?;
        send.finish().map_err(|e| AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;

        Ok(())
    }

    pub async fn recv_packet(&self) -> AdtpResult<AdtpPacket> {
        let (_, mut recv) =
            self.inner
                .accept_bi()
                .await
                .map_err(|e| AdtpError::QuicStreamError {
                    reason: e.to_string(),
                })?;
        let packet_bytes = read_framed(&mut recv).await?;

        AdtpPacket::from_bytes(Bytes::from(packet_bytes))
    }

    pub fn peer_addr(&self) -> String {
        self.inner.remote_address().to_string()
    }

    pub async fn close(&self) {
        self.inner.close(0u32.into(), b"goodbye");
    }
}

async fn write_framed(send: &mut SendStream, data: &[u8]) -> AdtpResult<()> {
    let len = data.len() as u64;
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;
    send.write_all(data)
        .await
        .map_err(|e| AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;

    Ok(())
}

async fn read_framed(recv: &mut RecvStream) -> AdtpResult<Vec<u8>> {
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;
    let len = u64::from_be_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data)
        .await
        .map_err(|e| AdtpError::QuicStreamError {
            reason: e.to_string(),
        })?;

    Ok(data)
}

fn generate_self_signed_cert() -> AdtpResult<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).map_err(|e| {
        AdtpError::TlsError {
            reason: e.to_string(),
        }
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
    let quic_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(crypto).map_err(|e| {
        AdtpError::TlsError {
            reason: e.to_string(),
        }
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

fn install_crypto_provider_once() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
