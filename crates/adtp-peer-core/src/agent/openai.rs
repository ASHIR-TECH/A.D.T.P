use crate::agent::provider::{LlmMessage, LlmResponse, Provider, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::time::Duration;

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    max_tokens: u32,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client");
        Self {
            api_key,
            model,
            max_tokens,
            client,
        }
    }

    fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn provider_name(&self) -> &str {
        "openai"
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
        if self.api_key.is_empty() {
            return Err("No API key configured — add one in Settings > Agents".to_string());
        }

        let mut openai_messages: Vec<serde_json::Value> = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt,
        })];

        for m in messages {
            openai_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content,
            }));
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "temperature": 0.7,
            "messages": openai_messages,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::convert_tools(tools));
            body["tool_choice"] = serde_json::json!("auto");
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    "OpenAI request timed out — check your internet connection".to_string()
                } else if e.is_connect() {
                    format!("OpenAI connection failed — check your internet connection and firewall ({}: {})", e.status().map(|s| s.as_u16()).unwrap_or(0), e)
                } else {
                    format!("OpenAI request failed: {}", e)
                }
            })?;

        let status = response.status();
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("OpenAI parse failed: {}", e))?;

        if !status.is_success() {
            let err_msg = response_json["error"]["message"]
                .as_str()
                .unwrap_or("unknown error");
            return Err(format!("OpenAI API error ({}): {}", status, err_msg));
        }

        let choice = response_json["choices"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or("No choices in response")?;

        let message = &choice["message"];
        let text_content = message["content"].as_str().unwrap_or("").to_string();
        let mut tool_calls = Vec::new();

        if let Some(tcs) = message["tool_calls"].as_array() {
            for tc in tcs {
                if let (Some(id), Some(function)) = (tc["id"].as_str(), tc["function"].as_object())
                {
                    let name = function["name"].as_str().unwrap_or("");
                    let args_str = function["arguments"].as_str().unwrap_or("{}");
                    if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_str) {
                        tool_calls.push(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments: args,
                        });
                    }
                }
            }
        }

        let finish_reason = choice["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        Ok(LlmResponse {
            content: text_content,
            tool_calls,
            stop_reason: finish_reason,
        })
    }
}
