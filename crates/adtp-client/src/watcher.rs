use crate::queue::{QueueItem, TransferQueue};
use adtp_core::types::WatchEntry;
use dashmap::DashMap;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct FolderWatcher {
    watches: Arc<DashMap<Uuid, WatchEntry>>,
    queue_tx: flume::Sender<QueueItem>,
}

impl FolderWatcher {
    pub fn new(queue_tx: flume::Sender<QueueItem>) -> Self {
        Self {
            watches: Arc::new(DashMap::new()),
            queue_tx,
        }
    }

    pub fn from_queue(queue: &TransferQueue) -> Self {
        Self::new(queue.sender())
    }

    pub fn register(&self, entry: WatchEntry) -> Uuid {
        let id = entry.id;
        self.watches.insert(id, entry);
        id
    }

    pub fn unregister(&self, watch_id: &Uuid) {
        self.watches.remove(watch_id);
    }

    pub fn list(&self) -> Vec<WatchEntry> {
        self.watches
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn start(self: Arc<Self>) {
        let watches = self.watches.clone();
        let queue_tx = self.queue_tx.clone();

        tokio::task::spawn_blocking(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
                Ok(watcher) => watcher,
                Err(error) => {
                    tracing::error!("Failed to create watcher: {}", error);
                    return;
                }
            };

            for entry in watches.iter() {
                let path = PathBuf::from(&entry.folder_path);
                if path.exists() {
                    if let Err(error) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                        tracing::warn!("Failed to watch {}: {}", path.display(), error);
                    } else {
                        tracing::info!("Watching: {}", path.display());
                    }
                }
            }

            for event in rx {
                match event {
                    Ok(event) if matches!(event.kind, notify::EventKind::Create(_)) => {
                        for path in event.paths {
                            if path.is_file() {
                                for entry in watches.iter() {
                                    let folder = PathBuf::from(&entry.folder_path);
                                    if path.starts_with(&folder) && entry.active {
                                        let agent = entry
                                            .trigger_action
                                            .clone()
                                            .unwrap_or_else(|| "default".to_string());
                                        let item = QueueItem::new_send(path.clone(), agent);
                                        if queue_tx.send(item).is_ok() {
                                            tracing::info!("Auto-queued: {}", path.display());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!("Watcher error: {}", error),
                }
            }
        });
    }
}
