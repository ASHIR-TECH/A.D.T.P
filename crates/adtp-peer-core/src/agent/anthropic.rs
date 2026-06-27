use async_trait::async_trait;
use crate::agent::provider::{LlmMessage, LlmResponse, ToolCall, ToolDefinition, Provider};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    max_tokens: u32,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }

    fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })
        }).collect()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn send_message(
        &self,
        messages: &[LlmMessage],
        system_prompt: &str,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, String> {
        let anthropic_messages: Vec<serde_json::Value> = messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": system_prompt,
            "messages": anthropic_messages,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::convert_tools(tools));
        }

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Anthropic request failed: {}", e))?;

        let status = response.status();
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Anthropic parse failed: {}", e))?;

        if !status.is_success() {
            let err_msg = response_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(format!("Anthropic API error ({}): {}", status, err_msg));
        }

        let content_blocks = response_json["content"].as_array().ok_or("No content in response")?;
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in content_blocks {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(text) = block["text"].as_str() {
                        text_content.push_str(text);
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name), Some(args)) = (
                        block["id"].as_str(),
                        block["name"].as_str(),
                        block["input"].as_object(),
                    ) {
                        tool_calls.push(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments: serde_json::Value::Object(args.clone()),
                        });
                    }
                }
                _ => {}
            }
        }

        let stop_reason = response_json["stop_reason"]
            .as_str()
            .unwrap_or("end_turn")
            .to_string();

        Ok(LlmResponse {
            content: text_content,
            tool_calls,
            stop_reason,
        })
    }
}
