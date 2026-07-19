use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTask {
    pub id: String,
    pub target_peer_id: String,
    pub instruction: String,
    pub task_id: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PendingTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PendingTaskStatus {
    Pending,
    Fired,
    Expired,
    Cancelled,
}

pub struct PendingTaskQueue {
    tasks: Arc<Mutex<HashMap<String, PendingTask>>>,
    expiry_hours: u64,
}

impl PendingTaskQueue {
    pub fn new(expiry_hours: u64) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            expiry_hours,
        }
    }

    pub async fn enqueue(
        &self,
        target_peer_id: String,
        instruction: String,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(self.expiry_hours as i64);

        let task = PendingTask {
            id: id.clone(),
            target_peer_id,
            instruction,
            task_id: None,
            queued_at: now,
            expires_at,
            status: PendingTaskStatus::Pending,
        };

        let mut tasks = self.tasks.lock().await;
        tasks.insert(id.clone(), task);
        Ok(id)
    }

    pub async fn fire(&self, id: &str) -> Option<PendingTask> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(id) {
            if task.status == PendingTaskStatus::Pending {
                let now = Utc::now();
                if now > task.expires_at {
                    task.status = PendingTaskStatus::Expired;
                    return None;
                }
                task.status = PendingTaskStatus::Fired;
                return Some(task.clone());
            }
        }
        None
    }

    pub async fn cancel(&self, id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(id) {
            if task.status == PendingTaskStatus::Pending {
                task.status = PendingTaskStatus::Cancelled;
                return true;
            }
        }
        false
    }

    pub async fn pending_for_peer(&self, peer_id: &str) -> Vec<PendingTask> {
        let mut tasks = self.tasks.lock().await;
        let now = Utc::now();
        let mut result = Vec::new();
        tasks.retain(|_, task| {
            if task.status == PendingTaskStatus::Pending && task.target_peer_id == peer_id {
                if now > task.expires_at {
                    task.status = PendingTaskStatus::Expired;
                    return false;
                }
                result.push(task.clone());
                false
            } else {
                task.status != PendingTaskStatus::Fired
            }
        });
        result
    }

    pub async fn all_pending(&self) -> Vec<PendingTask> {
        let mut tasks = self.tasks.lock().await;
        let now = Utc::now();
        let mut result = Vec::new();
        tasks.retain(|_, task| {
            if task.status == PendingTaskStatus::Pending {
                if now > task.expires_at {
                    task.status = PendingTaskStatus::Expired;
                    return false;
                }
                result.push(task.clone());
                true
            } else {
                task.status != PendingTaskStatus::Fired
            }
        });
        result
    }

    pub async fn fire_for_peer(&self, peer_id: &str) -> Vec<PendingTask> {
        self.pending_for_peer(peer_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_and_fire() {
        let queue = PendingTaskQueue::new(24);
        let id = queue
            .enqueue("peer-1".into(), "send file x".into())
            .await
            .unwrap();
        assert!(!id.is_empty());
        let fired = queue.fire(&id).await;
        assert!(fired.is_some());
        assert_eq!(fired.unwrap().status, PendingTaskStatus::Fired);
    }

    #[tokio::test]
    async fn test_cancel() {
        let queue = PendingTaskQueue::new(24);
        let id = queue
            .enqueue("peer-1".into(), "send file x".into())
            .await
            .unwrap();
        assert!(queue.cancel(&id).await);
        assert!(queue.fire(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_pending_for_peer() {
        let queue = PendingTaskQueue::new(24);
        queue
            .enqueue("peer-1".into(), "task 1".into())
            .await
            .unwrap();
        queue
            .enqueue("peer-2".into(), "task 2".into())
            .await
            .unwrap();
        queue
            .enqueue("peer-1".into(), "task 3".into())
            .await
            .unwrap();
        let peer1_tasks = queue.pending_for_peer("peer-1").await;
        assert_eq!(peer1_tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_expiry() {
        let queue = PendingTaskQueue::new(0);
        let id = queue.enqueue("peer-1".into(), "task".into()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(queue.fire(&id).await.is_none());
    }
}
