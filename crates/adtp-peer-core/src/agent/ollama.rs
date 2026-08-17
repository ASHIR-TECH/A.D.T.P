use crate::agent::provider::{LlmMessage, LlmResponse, Provider, ToolDefinition};
use async_trait::async_trait;
use std::time::Duration;

pub struct OllamaProvider {
    host: String,
    model: String,
    max_tokens: u32,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(host: String, model: String, max_tokens: u32) -> Self {
        let host = host.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client");
        Self {
            host,
            model,
            max_tokens,
            client,
        }
    }

    pub async fn check_available(&self) -> Result<bool, String> {
        let url = format!("{}/api/tags", self.host);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Ollama not reachable: {}", e))?;
        if !resp.status().is_success() {
            return Ok(false);
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Ollama parse error: {}", e))?;
        let models = json["models"]
            .as_array()
            .ok_or("Unexpected response format")?;
        Ok(models.iter().any(|m| {
            m["name"]
                .as_str()
                .map(|n| n.starts_with(&self.model))
                .unwrap_or(false)
        }))
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn send_message(
        &self,
        messages: &[LlmMessage],
        system_prompt: &str,
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse, String> {
        let mut ollama_messages: Vec<serde_json::Value> = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt,
        })];

        for m in messages {
            ollama_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content,
            }));
        }

        let body = serde_json::json!({
            "model": self.model,
            "messages": ollama_messages,
            "options": {
                "num_predict": self.max_tokens,
            },
            "stream": false,
        });

        let url = format!("{}/api/chat", self.host);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama request failed: {}", e))?;

        let status = response.status();
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Ollama parse failed: {}", e))?;

        if !status.is_success() {
            let err_msg = response_json["error"].as_str().unwrap_or("unknown error");
            return Err(format!("Ollama API error ({}): {}", status, err_msg));
        }

        let content = response_json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let done_reason = response_json["done_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        Ok(LlmResponse {
            content,
            tool_calls: vec![],
            stop_reason: done_reason,
        })
    }
}
