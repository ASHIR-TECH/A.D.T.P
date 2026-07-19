use adtp_core::errors::{AdtpError, AdtpResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Received,
    Accepted,
    Rejected,
    Processing,
    ArtifactReady,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Received => write!(f, "received"),
            TaskState::Accepted => write!(f, "accepted"),
            TaskState::Rejected => write!(f, "rejected"),
            TaskState::Processing => write!(f, "processing"),
            TaskState::ArtifactReady => write!(f, "artifact_ready"),
            TaskState::Completed => write!(f, "completed"),
            TaskState::Failed => write!(f, "failed"),
            TaskState::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TaskState {
    pub fn valid_next_states(&self) -> &[TaskState] {
        match self {
            TaskState::Received => &[TaskState::Accepted, TaskState::Rejected],
            TaskState::Accepted => &[TaskState::Processing, TaskState::Failed],
            TaskState::Rejected => &[],
            TaskState::Processing => &[TaskState::ArtifactReady, TaskState::Failed],
            TaskState::ArtifactReady => &[TaskState::Completed, TaskState::Failed],
            TaskState::Completed => &[],
            TaskState::Failed => &[],
            TaskState::Cancelled => &[],
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Rejected | TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    pub fn validate_transition(&self, to: &TaskState) -> AdtpResult<()> {
        if self.is_terminal() {
            return Err(AdtpError::TaskTerminal {
                state: self.to_string(),
            });
        }
        let allowed = self.valid_next_states();
        if !allowed.contains(to) {
            return Err(AdtpError::IllegalTaskTransition {
                from: self.to_string(),
                to: to.to_string(),
                reason: format!(
                    "Cannot transition from {} to {}. Allowed: {:?}",
                    self,
                    to,
                    allowed.iter().map(|s| s.to_string()).collect::<Vec<_>>()
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: Uuid,
    pub transfer_id: Uuid,
    pub session_id: Uuid,
    pub sender_peer_id: [u8; 16],
    pub receiver_peer_id: [u8; 16],
    pub instructions: String,
    pub expected_artifacts: Vec<String>,
    pub state: TaskState,
    pub reason: Option<String>,
    pub artifacts: Vec<ArtifactRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub artifact_name: String,
    pub transfer_id: Uuid,
    pub mime_type: String,
    pub file_size: u64,
    pub produced_at: DateTime<Utc>,
}

impl Task {
    pub fn new(
        transfer_id: Uuid,
        session_id: Uuid,
        sender_peer_id: [u8; 16],
        receiver_peer_id: [u8; 16],
        instructions: String,
        expected_artifacts: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            task_id: Uuid::new_v4(),
            transfer_id,
            session_id,
            sender_peer_id,
            receiver_peer_id,
            instructions,
            expected_artifacts,
            state: TaskState::Received,
            reason: None,
            artifacts: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn transition(&mut self, to: TaskState, reason: Option<String>) -> AdtpResult<()> {
        self.state.validate_transition(&to)?;
        self.state = to;
        self.reason = reason;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn add_artifact(
        &mut self,
        artifact_name: String,
        transfer_id: Uuid,
        mime_type: String,
        file_size: u64,
    ) -> AdtpResult<()> {
        if self.state != TaskState::Processing && self.state != TaskState::ArtifactReady {
            return Err(AdtpError::IllegalTaskTransition {
                from: self.state.to_string(),
                to: "artifact_ready".to_string(),
                reason: "Artifacts can only be added during processing or artifact_ready state"
                    .to_string(),
            });
        }
        if self
            .artifacts
            .iter()
            .any(|a| a.artifact_name == artifact_name)
        {
            return Err(AdtpError::ArtifactNotFound {
                task_id: self.task_id.to_string(),
                artifact_name,
            });
        }
        self.artifacts.push(ArtifactRef {
            artifact_name,
            transfer_id,
            mime_type,
            file_size,
            produced_at: Utc::now(),
        });
        self.state = TaskState::ArtifactReady;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub transfer_id: String,
    pub session_id: String,
    pub sender_peer_id: String,
    pub receiver_peer_id: String,
    pub instructions: String,
    pub expected_artifacts: Vec<String>,
    pub state: String,
    pub reason: Option<String>,
    pub artifacts: Vec<ArtifactRecord>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_name: String,
    pub transfer_id: String,
    pub mime_type: String,
    pub file_size: u64,
    pub produced_at: String,
}

impl From<Task> for TaskRecord {
    fn from(task: Task) -> Self {
        Self {
            task_id: task.task_id.to_string(),
            transfer_id: task.transfer_id.to_string(),
            session_id: task.session_id.to_string(),
            sender_peer_id: hex::encode(task.sender_peer_id),
            receiver_peer_id: hex::encode(task.receiver_peer_id),
            instructions: task.instructions,
            expected_artifacts: task.expected_artifacts,
            state: task.state.to_string(),
            reason: task.reason,
            artifacts: task.artifacts.into_iter().map(Into::into).collect(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        }
    }
}

impl From<ArtifactRef> for ArtifactRecord {
    fn from(a: ArtifactRef) -> Self {
        Self {
            artifact_name: a.artifact_name,
            transfer_id: a.transfer_id.to_string(),
            mime_type: a.mime_type,
            file_size: a.file_size,
            produced_at: a.produced_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStore {
    tasks: HashMap<String, Task>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn add(&mut self, task: Task) {
        self.tasks.insert(task.task_id.to_string(), task);
    }

    pub fn get(&self, task_id: &str) -> Option<&Task> {
        self.tasks.get(task_id)
    }

    pub fn get_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.get_mut(task_id)
    }

    pub fn list(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
        tasks
    }

    pub fn list_filtered(&self, peer_id: Option<&str>, state: Option<&str>) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| {
                let matches_peer = peer_id
                    .map(|p| {
                        hex::encode(t.sender_peer_id) == p || hex::encode(t.receiver_peer_id) == p
                    })
                    .unwrap_or(true);
                let matches_state = state.map(|s| t.state.to_string() == s).unwrap_or(true);
                matches_peer && matches_state
            })
            .collect()
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_task_initial_state() {
        let task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "process this file".into(),
            vec!["output.pdf".into()],
        );
        assert_eq!(task.state, TaskState::Received);
        assert!(!task.state.is_terminal());
    }

    #[test]
    fn test_valid_transition_received_to_accepted() {
        let mut task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "test".into(),
            vec![],
        );
        assert!(task.transition(TaskState::Accepted, None).is_ok());
        assert_eq!(task.state, TaskState::Accepted);
    }

    #[test]
    fn test_invalid_transition_received_to_completed() {
        let mut task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "test".into(),
            vec![],
        );
        let result = task.transition(TaskState::Completed, None);
        assert!(result.is_err());
        assert_eq!(task.state, TaskState::Received);
    }

    #[test]
    fn test_terminal_state_rejected() {
        let mut task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "test".into(),
            vec![],
        );
        task.transition(TaskState::Rejected, Some("not needed".into()))
            .unwrap();
        assert!(task.state.is_terminal());
        let result = task.transition(TaskState::Accepted, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_full_task_lifecycle() {
        let mut task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "convert file".into(),
            vec!["result.json".into()],
        );

        task.transition(TaskState::Accepted, None).unwrap();
        task.transition(TaskState::Processing, None).unwrap();

        let transfer_id = Uuid::new_v4();
        task.add_artifact(
            "result.json".into(),
            transfer_id,
            "application/json".into(),
            1024,
        )
        .unwrap();
        assert_eq!(task.state, TaskState::ArtifactReady);

        task.transition(TaskState::Completed, None).unwrap();
        assert!(task.state.is_terminal());
        assert_eq!(task.artifacts.len(), 1);
    }

    #[test]
    fn test_duplicate_artifact_rejected() {
        let mut task = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            [1u8; 16],
            [2u8; 16],
            "test".into(),
            vec!["a.txt".into(), "a.txt".into()],
        );
        task.transition(TaskState::Accepted, None).unwrap();
        task.transition(TaskState::Processing, None).unwrap();

        task.add_artifact("a.txt".into(), Uuid::new_v4(), "text/plain".into(), 100)
            .unwrap();

        let result = task.add_artifact("a.txt".into(), Uuid::new_v4(), "text/plain".into(), 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_task_store_list_filtered() {
        let mut store = TaskStore::new();
        let sender = [1u8; 16];
        let receiver = [2u8; 16];

        let t1 = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            sender,
            receiver,
            "task 1".into(),
            vec![],
        );
        let t2 = Task::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            receiver,
            [3u8; 16],
            "task 2".into(),
            vec![],
        );
        store.add(t1);
        store.add(t2);

        let filtered = store.list_filtered(Some(&hex::encode(sender)), None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].instructions, "task 1");
    }

    #[test]
    fn test_state_display() {
        assert_eq!(TaskState::Received.to_string(), "received");
        assert_eq!(TaskState::Completed.to_string(), "completed");
        assert_eq!(TaskState::Failed.to_string(), "failed");
        assert_eq!(TaskState::Cancelled.to_string(), "cancelled");
    }
}
