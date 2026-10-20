//! Built-in shared assistant ("Base AI").
//!
//! This is the same shared assistant the MARS mobile app uses as its
//! quick-response fallback: the `qwen-chat` Supabase edge function, which holds
//! the real provider key server-side and enforces a per-user daily cap. The
//! desktop holds only the publishable (`anon`) key, which is designed to be
//! public.
//!
//! The edge function requires a signed-in Supabase user, so this provider
//! self-provisions an *anonymous* Supabase session on first use and refreshes
//! it transparently — no login UI for the user. That requires "Anonymous
//! sign-ins" to be enabled for the Supabase project; when it isn't, the
//! provider returns actionable guidance instead of a raw error.
//!
//! The assistant is text-only (no tool calls): it is a fallback for users who
//! haven't configured a provider of their own, not a replacement for a
//! bring-your-own-key agent.
//!
//! The shared function hardcodes a "MARS Co-Pilot" persona, so the desktop asks
//! for an ADTP identity by sending a platform directive through the function's
//! string `context` field (which the function appends to its system prompt).
//! Mobile sends no context and keeps the MARS identity — see
//! [`DEFAULT_BASE_AI_PERSONA`] and `ADTP_BASE_AI_PERSONA`.

use crate::agent::provider::{LlmMessage, LlmResponse, Provider, ToolDefinition};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Provider id used by [`crate::agent::runtime::AgentRuntime`] and stored on the
/// auto-installed agent entry.
pub const BASE_PROVIDER: &str = "base";
/// Stable id for the auto-installed shared agent.
pub const BASE_AGENT_ID: &str = "base-ai";

/// Default shared assistant endpoint (MARS Co-Pilot / `qwen-chat`). Public URL;
/// override with `ADTP_BASE_AI_URL` to self-host.
pub const DEFAULT_BASE_AI_URL: &str =
    "https://yiaknrtvtgtmbgzhdlru.supabase.co/functions/v1/qwen-chat";
/// Supabase project used for the anonymous session. Override with
/// `ADTP_BASE_AI_SUPABASE_URL`.
pub const DEFAULT_BASE_AI_SUPABASE_URL: &str = "https://yiaknrtvtgtmbgzhdlru.supabase.co";
/// Publishable (`anon`) key. Public by design; override with `ADTP_BASE_AI_KEY`.
pub const DEFAULT_BASE_AI_KEY: &str = "sb_publishable_6IxBQKWTJaEom0SXyM0agA_oeLroSY2";

/// Platform directive sent as the function's `context` so the shared assistant
/// presents itself as the ADTP Assistant on the desktop. The edge function
/// appends a string `context` to its system prompt, which is how the PC asks
/// for the ADTP identity without changing MARS mobile behaviour. Override with
/// `ADTP_BASE_AI_PERSONA`; set it to an empty value to send no persona.
pub const DEFAULT_BASE_AI_PERSONA: &str =
    "Platform: this request comes from the ADTP desktop app, not the MARS mobile app. \
     On this platform you are the ADTP Assistant; the MARS Co-Pilot branding and the \
     name \"MARS Co-Pilot\" do not apply. If asked who or what you are, say you are the \
     ADTP Assistant.";

/// Builds the shared-function payload. The function appends a string `context`
/// to its system prompt, so the desktop embeds the platform persona there.
pub fn build_chat_body(text: &str, persona: Option<&str>) -> serde_json::Value {
    match persona {
        Some(persona) if !persona.trim().is_empty() => {
            serde_json::json!({ "text": text, "context": persona })
        }
        _ => serde_json::json!({ "text": text }),
    }
}

/// A cached Supabase session used to authenticate against the shared function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseSession {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix seconds after which the access token should be considered stale.
    pub expires_at: i64,
}

/// The function is stateless and takes a single `text`, so only the most recent
/// user turn is sent — matching how MARS calls it. Earlier turns stay in the
/// desktop conversation for display.
pub fn last_user_text(messages: &[LlmMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
}

/// Parses a Supabase auth response into a session, expiring a minute early so a
/// token can't go stale mid-request.
pub fn session_from_json(value: &serde_json::Value, now: i64) -> Result<BaseSession, String> {
    let access_token = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("Shared AI sign-in response did not include an access token")?
        .to_string();
    let refresh_token = value
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let expires_in = value
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    Ok(BaseSession {
        access_token,
        refresh_token,
        expires_at: now + expires_in - 60,
    })
}

/// Turns an error body into something the user can act on.
pub fn function_error(status: u16, body: &serde_json::Value) -> String {
    let message = body
        .get("message")
        .or_else(|| body.get("error"))
        .or_else(|| body.get("msg"))
        .and_then(|v| v.as_str());

    if let Some(message) = message {
        return message.to_string();
    }

    match status {
        429 => "You've reached today's shared-assistant limit. Try again tomorrow, or add your own API key in Settings \u{203a} Agents.".to_string(),
        500 => "The shared assistant is not configured on the server.".to_string(),
        502..=504 => "The shared assistant is temporarily unavailable. Try again shortly.".to_string(),
        _ => format!("Shared assistant request failed (HTTP {status})"),
    }
}

fn load_session_from(path: &Path) -> Option<BaseSession> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_session_to(path: &Path, session: &BaseSession) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(session) {
        let _ = std::fs::write(path, content);
    }
}

pub struct BaseProvider {
    endpoint: String,
    supabase_url: String,
    api_key: String,
    model_label: String,
    persona: Option<String>,
    client: reqwest::Client,
}

impl BaseProvider {
    pub fn new() -> Self {
        let persona = match std::env::var("ADTP_BASE_AI_PERSONA") {
            Ok(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Err(_) => Some(DEFAULT_BASE_AI_PERSONA.to_string()),
        };
        Self {
            endpoint: std::env::var("ADTP_BASE_AI_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_AI_URL.to_string()),
            supabase_url: std::env::var("ADTP_BASE_AI_SUPABASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_AI_SUPABASE_URL.to_string()),
            api_key: std::env::var("ADTP_BASE_AI_KEY")
                .unwrap_or_else(|_| DEFAULT_BASE_AI_KEY.to_string()),
            model_label: std::env::var("ADTP_BASE_AI_MODEL")
                .unwrap_or_else(|_| "mars-base".to_string()),
            persona,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(120))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    fn session_path() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("ADTP_BASE_AI_SESSION_PATH") {
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
        dirs::config_dir().map(|p| p.join("adtp").join("base_ai_session.json"))
    }

    fn load_session() -> Option<BaseSession> {
        Self::session_path().and_then(|p| load_session_from(&p))
    }

    fn store_session(session: &BaseSession) {
        if let Some(path) = Self::session_path() {
            save_session_to(&path, session);
        }
    }

    fn auth_url(&self, path: &str) -> String {
        format!("{}{}", self.supabase_url.trim_end_matches('/'), path)
    }

    /// Returns a usable access token, refreshing or signing in anonymously as
    /// needed.
    async fn access_token(&self) -> Result<String, String> {
        let now = chrono::Utc::now().timestamp();
        let cached = Self::load_session();

        if let Some(session) = &cached {
            if session.expires_at > now {
                return Ok(session.access_token.clone());
            }
        }

        if let Some(refresh_token) = cached.as_ref().and_then(|s| s.refresh_token.clone()) {
            match self.refresh(&refresh_token).await {
                Ok(session) => {
                    Self::store_session(&session);
                    return Ok(session.access_token);
                }
                Err(error) => {
                    tracing::warn!("base AI session refresh failed, re-signing in: {error}");
                }
            }
        }

        let session = self.sign_in_anonymous().await?;
        Self::store_session(&session);
        Ok(session.access_token)
    }

    async fn sign_in_anonymous(&self) -> Result<BaseSession, String> {
        let response = self
            .client
            .post(self.auth_url("/auth/v1/signup"))
            .header("apikey", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("Could not reach the shared assistant: {e}"))?;

        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().await.unwrap_or(serde_json::Value::Null);

        if !(200..300).contains(&status) {
            let code = body
                .get("error_code")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if code == "anonymous_provider_disabled" {
                return Err(
                    "The shared assistant needs anonymous sign-ins enabled in Supabase, \
                     or add your own API key in Settings \u{203a} Agents."
                        .to_string(),
                );
            }
            return Err(function_error(status, &body));
        }

        session_from_json(&body, chrono::Utc::now().timestamp())
    }

    async fn refresh(&self, refresh_token: &str) -> Result<BaseSession, String> {
        let response = self
            .client
            .post(self.auth_url("/auth/v1/token?grant_type=refresh_token"))
            .header("apikey", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| format!("Could not reach the shared assistant: {e}"))?;

        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().await.unwrap_or(serde_json::Value::Null);

        if !(200..300).contains(&status) {
            return Err(function_error(status, &body));
        }

        session_from_json(&body, chrono::Utc::now().timestamp())
    }

    /// Single function call. Errors keep the status so the caller can decide to
    /// retry once on 401.
    async fn chat(
        &self,
        token: &str,
        text: &str,
    ) -> Result<serde_json::Value, (u16, serde_json::Value)> {
        let response = self
            .client
            .post(&self.endpoint)
            .header("apikey", &self.api_key)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&build_chat_body(text, self.persona.as_deref()))
            .send()
            .await
            .map_err(|e| {
                (
                    0u16,
                    serde_json::json!({ "message": format!("Could not reach the shared assistant: {e}") }),
                )
            })?;

        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().await.unwrap_or(serde_json::Value::Null);
        if (200..300).contains(&status) {
            Ok(body)
        } else {
            Err((status, body))
        }
    }

    fn to_response(&self, body: &serde_json::Value) -> LlmResponse {
        let content = body
            .get("reply")
            .and_then(|v| v.as_str())
            .or_else(|| body.get("response").and_then(|v| v.as_str()))
            .or_else(|| body.get("text").and_then(|v| v.as_str()))
            .unwrap_or_default()
            .to_string();
        LlmResponse {
            content,
            tool_calls: vec![],
            stop_reason: "end_turn".into(),
        }
    }
}

impl Default for BaseProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for BaseProvider {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        _system_prompt: &str,
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse, String> {
        let text = last_user_text(messages)
            .ok_or("The shared assistant needs a user message to answer")?;

        let token = self.access_token().await?;
        match self.chat(&token, &text).await {
            Ok(body) => Ok(self.to_response(&body)),
            Err((401, _)) => {
                // Session may have expired between the expiry check and the
                // call — mint a fresh anonymous session and retry once.
                let session = self.sign_in_anonymous().await?;
                Self::store_session(&session);
                match self.chat(&session.access_token, &text).await {
                    Ok(body) => Ok(self.to_response(&body)),
                    Err((status, body)) => Err(function_error(status, &body)),
                }
            }
            Err((status, body)) => Err(function_error(status, &body)),
        }
    }

    fn provider_name(&self) -> &str {
        BASE_PROVIDER
    }

    fn model_name(&self) -> &str {
        &self.model_label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> LlmMessage {
        LlmMessage {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_last_user_text_picks_latest_user_turn() {
        let messages = vec![
            msg("user", "first"),
            msg("assistant", "reply"),
            msg("user", "second"),
        ];
        assert_eq!(last_user_text(&messages), Some("second".to_string()));
    }

    #[test]
    fn test_last_user_text_none_without_user_turn() {
        assert_eq!(last_user_text(&[msg("assistant", "hi")]), None);
    }

    #[test]
    fn test_session_from_json_computes_expiry() {
        let body = serde_json::json!({
            "access_token": "tok",
            "refresh_token": "ref",
            "expires_in": 3600
        });
        let session = session_from_json(&body, 1_000).unwrap();
        assert_eq!(session.access_token, "tok");
        assert_eq!(session.refresh_token.as_deref(), Some("ref"));
        assert_eq!(session.expires_at, 1_000 + 3600 - 60);
    }

    #[test]
    fn test_session_from_json_requires_access_token() {
        let body = serde_json::json!({ "refresh_token": "ref" });
        assert!(session_from_json(&body, 0).is_err());
    }

    #[test]
    fn test_function_error_prefers_server_message() {
        let body = serde_json::json!({ "message": "Daily limit reached" });
        assert_eq!(function_error(429, &body), "Daily limit reached");
    }

    #[test]
    fn test_function_error_falls_back_per_status() {
        assert!(function_error(429, &serde_json::Value::Null).contains("limit"));
        assert!(function_error(502, &serde_json::Value::Null).contains("unavailable"));
        assert!(function_error(418, &serde_json::Value::Null).contains("418"));
    }

    #[test]
    fn test_session_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("base_ai_session.json");
        let session = BaseSession {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: 12345,
        };
        assert!(load_session_from(&path).is_none());
        save_session_to(&path, &session);
        let loaded = load_session_from(&path).unwrap();
        assert_eq!(loaded.access_token, "a");
        assert_eq!(loaded.refresh_token.as_deref(), Some("r"));
        assert_eq!(loaded.expires_at, 12345);
    }

    #[test]
    fn test_default_provider_uses_public_defaults() {
        let provider = BaseProvider::new();
        assert_eq!(provider.provider_name(), BASE_PROVIDER);
        assert!(provider.endpoint.contains("qwen-chat"));
        assert!(!provider.api_key.is_empty());
    }

    #[test]
    fn test_build_chat_body_includes_persona_as_context() {
        let body = build_chat_body("hello", Some("be the ADTP Assistant"));
        assert_eq!(body["text"], "hello");
        assert_eq!(body["context"], "be the ADTP Assistant");
    }

    #[test]
    fn test_build_chat_body_omits_blank_or_missing_persona() {
        assert!(build_chat_body("hello", None).get("context").is_none());
        assert!(build_chat_body("hello", Some("   "))
            .get("context")
            .is_none());
    }

    #[test]
    fn test_default_persona_asks_for_adtp_identity() {
        assert!(DEFAULT_BASE_AI_PERSONA.contains("ADTP"));
        assert!(DEFAULT_BASE_AI_PERSONA.contains("MARS"));
    }

    /// Live probe against the real shared endpoint. Ignored by default so the
    /// suite stays offline; run with `--ignored`. Either anonymous sign-ins are
    /// enabled (we get a session) or the project reports them disabled (we map
    /// it to actionable guidance) — both are correct outcomes.
    #[tokio::test]
    #[ignore = "hits the live shared endpoint"]
    async fn live_anonymous_sign_in_reports_disabled_or_succeeds() {
        let provider = BaseProvider::new();
        match provider.sign_in_anonymous().await {
            Ok(session) => assert!(!session.access_token.is_empty()),
            Err(error) => assert!(
                error.contains("anonymous sign-ins") || error.contains("Could not reach"),
                "unexpected error: {error}"
            ),
        }
    }
}
