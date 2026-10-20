use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: String,
}

#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        system_prompt: &str,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, String>;
    fn provider_name(&self) -> &str;
    fn model_name(&self) -> &str;
}
