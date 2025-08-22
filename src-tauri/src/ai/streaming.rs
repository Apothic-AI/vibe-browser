use super::{StreamingEvent, StreamingEventType, ComponentGenerationResponse};
use anyhow::Result;
use serde_json;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

/// Manager for handling streaming events and communication with frontend
#[derive(Clone)]
pub struct StreamingManager {
    app_handle: AppHandle,
}

impl StreamingManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Emit a streaming event to the frontend
    pub fn emit_event(&self, event: StreamingEvent) -> Result<()> {
        self.app_handle.emit("ai_streaming_event", &event)
            .map_err(|e| anyhow::anyhow!("Failed to emit streaming event: {}", e))?;
        Ok(())
    }

    /// Create a callback function for streaming updates
    pub fn create_callback(&self) -> impl Fn(StreamingEvent) + Send + 'static {
        let app_handle = self.app_handle.clone();
        move |event: StreamingEvent| {
            if let Err(e) = app_handle.emit("ai_streaming_event", &event) {
                log::error!("Failed to emit streaming event: {}", e);
            }
        }
    }

    /// Start a streaming session and return a channel for sending events
    pub fn start_streaming_session(&self, session_id: String) -> mpsc::UnboundedSender<StreamingEvent> {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamingEvent>();
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = app_handle.emit("ai_streaming_event", &event) {
                    log::error!("Failed to emit streaming event in session {}: {}", session_id, e);
                }

                // If this is a Complete or Error event, end the session
                match event.event_type {
                    StreamingEventType::Complete | StreamingEventType::Error => {
                        log::info!("Streaming session {} completed", session_id);
                        break;
                    }
                    _ => {}
                }
            }
        });

        tx
    }
}

/// Builder for creating streaming events
pub struct StreamingEventBuilder {
    event_type: StreamingEventType,
    data: serde_json::Value,
}

impl StreamingEventBuilder {
    pub fn progress(stage: &str, progress: f32) -> Self {
        Self {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({
                "stage": stage,
                "progress": progress
            }),
        }
    }

    pub fn partial_result(content: &str) -> Self {
        Self {
            event_type: StreamingEventType::PartialResult,
            data: serde_json::json!({
                "content": content
            }),
        }
    }

    pub fn complete(response: &ComponentGenerationResponse) -> Self {
        Self {
            event_type: StreamingEventType::Complete,
            data: serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({})),
        }
    }

    pub fn error(message: &str, details: Option<&str>) -> Self {
        Self {
            event_type: StreamingEventType::Error,
            data: serde_json::json!({
                "message": message,
                "details": details
            }),
        }
    }

    pub fn custom(event_type: StreamingEventType, data: serde_json::Value) -> Self {
        Self { event_type, data }
    }

    pub fn build(self) -> StreamingEvent {
        StreamingEvent {
            event_type: self.event_type,
            data: self.data,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Utility functions for common streaming patterns
pub mod utils {
    use super::*;

    /// Create a progress event
    pub fn progress_event(stage: &str, progress: f32) -> StreamingEvent {
        StreamingEventBuilder::progress(stage, progress).build()
    }

    /// Create a partial result event
    pub fn partial_result_event(content: &str) -> StreamingEvent {
        StreamingEventBuilder::partial_result(content).build()
    }

    /// Create a completion event
    pub fn completion_event(response: &ComponentGenerationResponse) -> StreamingEvent {
        StreamingEventBuilder::complete(response).build()
    }

    /// Create an error event
    pub fn error_event(message: &str, details: Option<&str>) -> StreamingEvent {
        StreamingEventBuilder::error(message, details).build()
    }

    /// Send a series of progress updates
    pub async fn send_progress_sequence<F>(
        callback: &F,
        stages: Vec<(&str, f32)>,
        delay_ms: Option<u64>,
    ) where
        F: Fn(StreamingEvent) + Send + Sync,
    {
        for (stage, progress) in stages {
            callback(progress_event(stage, progress));
            
            if let Some(delay) = delay_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ValidationStatus};

    #[test]
    fn test_streaming_event_builder() {
        let event = StreamingEventBuilder::progress("generation", 0.5).build();
        assert!(matches!(event.event_type, StreamingEventType::Progress));
        assert_eq!(event.data["stage"], "generation");
        assert_eq!(event.data["progress"], 0.5);
    }

    #[test]
    fn test_error_event_builder() {
        let event = StreamingEventBuilder::error("Test error", Some("Additional details")).build();
        assert!(matches!(event.event_type, StreamingEventType::Error));
        assert_eq!(event.data["message"], "Test error");
        assert_eq!(event.data["details"], "Additional details");
    }

    #[test]
    fn test_completion_event_builder() {
        let response = ComponentGenerationResponse {
            component_code: "test code".to_string(),
            component_name: "TestComponent".to_string(),
            description: "Test description".to_string(),
            dependencies: vec!["solid-js".to_string()],
            validation_status: ValidationStatus::Valid,
        };

        let event = StreamingEventBuilder::complete(&response).build();
        assert!(matches!(event.event_type, StreamingEventType::Complete));
        assert_eq!(event.data["component_name"], "TestComponent");
    }
}