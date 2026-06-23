use adtp_core::types::TraceId;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub peer_id: String,
    pub task_id: String,
    pub operation: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Tracer {
    pub peer_id: String,
    pub otel_endpoint: Option<String>,
    active_spans: HashMap<String, Span>,
}

impl Tracer {
    pub fn new(peer_id: String, otel_endpoint: Option<String>) -> Self {
        Self {
            peer_id,
            otel_endpoint,
            active_spans: HashMap::new(),
        }
    }

    pub fn start_span(
        &mut self,
        trace_id: TraceId,
        task_id: &str,
        operation: &str,
        parent_span_id: Option<String>,
    ) -> Span {
        let span_id = uuid::Uuid::new_v4().to_string();
        let span = Span {
            span_id: span_id.clone(),
            trace_id: trace_id.to_hex(),
            parent_span_id,
            peer_id: self.peer_id.clone(),
            task_id: task_id.to_string(),
            operation: operation.to_string(),
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            attributes: HashMap::new(),
        };
        self.active_spans.insert(span_id.clone(), span.clone());
        span
    }

    pub fn end_span(&mut self, span_id: &str) {
        if let Some(span) = self.active_spans.get_mut(span_id) {
            span.ended_at = Some(Utc::now().to_rfc3339());
            if let Some(endpoint) = &self.otel_endpoint {
                if let Err(e) = export_via_otlp(span, endpoint) {
                    tracing::warn!("OTLP export failed: {e}");
                }
            }
        }
    }

    pub fn add_attribute(&mut self, span_id: &str, key: &str, value: &str) {
        if let Some(span) = self.active_spans.get_mut(span_id) {
            span.attributes.insert(key.to_string(), value.to_string());
        }
    }

    pub fn span(&self, span_id: &str) -> Option<&Span> {
        self.active_spans.get(span_id)
    }
}

fn export_via_otlp(_span: &Span, _endpoint: &str) -> Result<(), String> {
    // OTLP export is placeholder for Phase 9.
    // Actual implementation would use opentelemetry crate with feature flag.
    // For now, just log the trace data.
    tracing::debug!(
        "OTLP export (placeholder): trace_id={}, span_id={}, operation={}",
        _span.trace_id,
        _span.span_id,
        _span.operation
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adtp_core::types::TraceId;

    #[test]
    fn test_trace_id_generation() {
        let id1 = TraceId::new();
        let id2 = TraceId::new();
        assert_ne!(id1.to_hex(), id2.to_hex());
        assert_eq!(id1.to_hex().len(), 64);
    }

    #[test]
    fn test_span_lifecycle() {
        let mut tracer = Tracer::new("peer_a".to_string(), None);
        let trace_id = TraceId::new();

        let span = tracer.start_span(trace_id, "task-123", "relay_forward", None);
        let span_id = span.span_id.clone();
        assert_eq!(span.operation, "relay_forward");
        assert!(span.ended_at.is_none());

        tracer.add_attribute(&span_id, "hop_count", "2");
        tracer.end_span(&span_id);

        let ended = tracer.span(&span_id).unwrap();
        assert!(ended.ended_at.is_some());
        assert_eq!(ended.attributes.get("hop_count").unwrap(), "2");
    }

    #[test]
    fn test_trace_id_display() {
        let trace_id = TraceId::from_bytes([0xAB; 32]);
        let hex = format!("{trace_id}");
        assert_eq!(hex.len(), 64);
        assert!(hex.starts_with("ab"));
    }
}
