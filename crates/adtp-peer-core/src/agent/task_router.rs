use crate::agent::artifact_writer::ArtifactWriter;
use crate::agent::runtime::AgentRuntime;
use std::sync::Arc;

pub struct TaskRouter {
    runtime: Arc<AgentRuntime>,
}

impl TaskRouter {
    pub fn new(runtime: Arc<AgentRuntime>) -> Self {
        Self { runtime }
    }

    pub async fn handle_incoming_task(
        &self,
        task_instruction: &str,
        file_context: Option<&str>,
        transfer_history: Option<&str>,
    ) -> Result<String, String> {
        let state = self.runtime.state().await;
        match state {
            crate::agent::runtime::AgentState::Inactive => {
                return Err("Agent is not active. Cannot process task.".into());
            }
            crate::agent::runtime::AgentState::Error(e) => {
                return Err(format!("Agent is in error state: {}", e));
            }
            _ => {}
        }

        let mut message = format!("You have been assigned a task: {}", task_instruction);

        if let Some(ctx) = file_context {
            message.push_str("\n\nFile context:\n");
            message.push_str(ctx);
        }

        if let Some(history) = transfer_history {
            message.push_str("\n\nTransfer history:\n");
            message.push_str(history);
        }

        message.push_str("\n\nPlease complete this task. If you need to read files, use read_file. If you need to create an output file, describe what you produced so I can save it as an artifact.");

        let response = self.runtime.send_message(&message).await?;

        let writer = ArtifactWriter::new();
        let artifact_path = writer.write_artifact(&response).await?;

        Ok(format!(
            "{}\n\nArtifact written to: {}",
            response, artifact_path
        ))
    }
}
