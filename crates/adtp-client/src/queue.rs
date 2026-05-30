use adtp_core::types::{Priority, TransferType};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub agent_name: String,
    pub message: Option<String>,
    pub priority: Priority,
    pub transfer_type: TransferType,
    pub destination: Option<String>,
}

impl QueueItem {
    pub fn new_send(file_path: PathBuf, agent_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            file_path,
            agent_name,
            message: None,
            priority: Priority::Normal,
            transfer_type: TransferType::Send,
            destination: None,
        }
    }
}

pub struct TransferQueue {
    sender: flume::Sender<QueueItem>,
    receiver: flume::Receiver<QueueItem>,
}

impl TransferQueue {
    pub fn new() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }

    pub fn enqueue(&self, item: QueueItem) -> adtp_core::errors::AdtpResult<()> {
        self.sender
            .send(item)
            .map_err(|e| adtp_core::errors::AdtpError::Internal {
                reason: e.to_string(),
            })
    }

    pub fn sender(&self) -> flume::Sender<QueueItem> {
        self.sender.clone()
    }

    pub fn receiver(&self) -> flume::Receiver<QueueItem> {
        self.receiver.clone()
    }
}

impl Default for TransferQueue {
    fn default() -> Self {
        Self::new()
    }
}
