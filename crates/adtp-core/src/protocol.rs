use bytes::{Buf, BufMut, Bytes, BytesMut};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{AdtpError, AdtpResult};
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    pub magic: u32,
    pub version: u8,
    pub transfer_type: TransferType,
    pub priority: Priority,
    pub flags: u8,
    pub session_id: Uuid,
    pub transfer_id: Uuid,
    pub sender_peer_id: [u8; PEER_ID_SIZE],
    pub receiver_peer_id: [u8; PEER_ID_SIZE],
    pub timestamp_ns: u64,
    pub context_len: u32,
    pub payload_len: u64,
    pub nonce: [u8; AES_NONCE_SIZE],
    pub content_hash: [u8; BLAKE3_HASH_SIZE],
}

impl PacketHeader {
    pub fn new(
        transfer_type: TransferType,
        session_id: Uuid,
        transfer_id: Uuid,
        sender_peer_id: [u8; PEER_ID_SIZE],
        receiver_peer_id: [u8; PEER_ID_SIZE],
        nonce: [u8; AES_NONCE_SIZE],
        content_hash: [u8; BLAKE3_HASH_SIZE],
        context_len: u32,
        payload_len: u64,
    ) -> Self {
        Self {
            magic: ADTP_MAGIC,
            version: ADTP_VERSION,
            transfer_type,
            priority: Priority::Normal,
            flags: 0x00,
            session_id,
            transfer_id,
            sender_peer_id,
            receiver_peer_id,
            timestamp_ns: Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
            context_len,
            payload_len,
            nonce,
            content_hash,
        }
    }

    pub fn validate(&self) -> AdtpResult<()> {
        if self.magic != ADTP_MAGIC {
            return Err(AdtpError::InvalidMagic { got: self.magic });
        }
        if self.version != ADTP_VERSION {
            return Err(AdtpError::UnsupportedVersion {
                version: self.version,
            });
        }
        if self.payload_len > ADTP_MAX_FILE_SIZE {
            return Err(AdtpError::PacketTooLarge {
                size: self.payload_len,
                max: ADTP_MAX_FILE_SIZE,
            });
        }
        if self.context_len as usize > ADTP_MAX_CONTEXT_SIZE {
            return Err(AdtpError::ContextTooLarge {
                size: self.context_len as usize,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AdtpPacket {
    pub header: PacketHeader,
    pub context: Bytes,
    pub payload: Bytes,
    pub signature: [u8; ED25519_SIG_SIZE],
}

impl AdtpPacket {
    pub fn to_bytes(&self) -> AdtpResult<Bytes> {
        let header_bytes = bincode::serialize(&self.header).map_err(AdtpError::Serialisation)?;
        let total = std::mem::size_of::<u32>()
            + header_bytes.len()
            + self.context.len()
            + self.payload.len()
            + ED25519_SIG_SIZE;
        let mut buf = BytesMut::with_capacity(total);

        buf.put_u32(header_bytes.len() as u32);
        buf.put_slice(&header_bytes);
        buf.put_slice(&self.context);
        buf.put_slice(&self.payload);
        buf.put_slice(&self.signature);

        Ok(buf.freeze())
    }

    pub fn from_bytes(mut data: Bytes) -> AdtpResult<Self> {
        if data.remaining() < std::mem::size_of::<u32>() {
            return Err(AdtpError::MalformedPacket {
                reason: "Too short to contain header length".to_string(),
            });
        }

        let header_len = data.get_u32() as usize;
        if data.remaining() < header_len {
            return Err(AdtpError::MalformedPacket {
                reason: format!(
                    "Expected {} header bytes, got {}",
                    header_len,
                    data.remaining()
                ),
            });
        }

        let header_bytes = data.copy_to_bytes(header_len);
        let header: PacketHeader =
            bincode::deserialize(&header_bytes).map_err(AdtpError::Serialisation)?;
        header.validate()?;

        let expected_remaining =
            header.context_len as usize + header.payload_len as usize + ED25519_SIG_SIZE;
        if data.remaining() < expected_remaining {
            return Err(AdtpError::MalformedPacket {
                reason: format!(
                    "Expected {} body bytes, got {}",
                    expected_remaining,
                    data.remaining()
                ),
            });
        }

        let context = data.copy_to_bytes(header.context_len as usize);
        let payload = data.copy_to_bytes(header.payload_len as usize);
        let mut signature = [0u8; ED25519_SIG_SIZE];
        data.copy_to_slice(&mut signature);

        Ok(Self {
            header,
            context,
            payload,
            signature,
        })
    }

    pub fn signable_bytes(&self) -> AdtpResult<Vec<u8>> {
        let header_bytes = bincode::serialize(&self.header).map_err(AdtpError::Serialisation)?;
        let context_hash = blake3::hash(&self.context);
        let payload_hash = blake3::hash(&self.payload);

        let mut signable =
            Vec::with_capacity(header_bytes.len() + BLAKE3_HASH_SIZE + BLAKE3_HASH_SIZE);
        signable.extend_from_slice(&header_bytes);
        signable.extend_from_slice(context_hash.as_bytes());
        signable.extend_from_slice(payload_hash.as_bytes());

        Ok(signable)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnvelope {
    pub file_meta: FileMeta,
    pub context: ContextBlock,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_packet() -> AdtpPacket {
        let session_id = Uuid::new_v4();
        let transfer_id = Uuid::new_v4();
        let nonce = [0u8; AES_NONCE_SIZE];
        let content_hash = [0u8; BLAKE3_HASH_SIZE];
        let context = Bytes::from(b"{}".to_vec());
        let payload = Bytes::from(b"encrypted bytes here".to_vec());
        let sender_peer_id = [1u8; PEER_ID_SIZE];
        let receiver_peer_id = [2u8; PEER_ID_SIZE];
        let header = PacketHeader::new(
            TransferType::Transfer,
            session_id,
            transfer_id,
            sender_peer_id,
            receiver_peer_id,
            nonce,
            content_hash,
            context.len() as u32,
            payload.len() as u64,
        );

        AdtpPacket {
            header,
            context,
            payload,
            signature: [0u8; ED25519_SIG_SIZE],
        }
    }

    #[test]
    fn test_packet_serialise_deserialise() {
        let packet = make_test_packet();
        let bytes = packet.to_bytes().unwrap();
        let decoded = AdtpPacket::from_bytes(bytes).unwrap();

        assert_eq!(decoded.header.magic, ADTP_MAGIC);
        assert_eq!(decoded.header.version, ADTP_VERSION);
        assert_eq!(decoded.payload, packet.payload);
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut packet = make_test_packet();
        packet.header.magic = 0xDEADBEEF;
        let bytes = packet.to_bytes().unwrap();
        let result = AdtpPacket::from_bytes(bytes);

        assert!(result.is_err());
    }

    #[test]
    fn test_signable_bytes_deterministic() {
        let packet = make_test_packet();
        let sig1 = packet.signable_bytes().unwrap();
        let sig2 = packet.signable_bytes().unwrap();

        assert_eq!(sig1, sig2);
    }
}
