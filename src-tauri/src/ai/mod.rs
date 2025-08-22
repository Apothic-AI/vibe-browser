pub mod pocketflow;
pub mod providers;
pub mod streaming;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use async_trait::async_trait;

/// Core trait for workflow nodes in the AI system
#[async_trait]
pub trait WorkflowNode: Send + Sync {
    /// Execute the node with given inputs and return outputs
    async fn execute(&self, inputs: HashMap<String, serde_json::Value>) -> Result<HashMap<String, serde_json::Value>>;
    
    /// Get the node's unique identifier
    fn node_id(&self) -> &str;
    
    /// Get the node's type/category
    fn node_type(&self) -> &str;
    
    /// Validate inputs before execution
    fn validate_inputs(&self, inputs: &HashMap<String, serde_json::Value>) -> Result<()>;
}

/// Configuration for AI providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProviderConfig {
    pub provider_type: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Request for component generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentGenerationRequest {
    pub requirements: String,
    pub component_type: Option<String>,
    pub style_framework: Option<String>,
    pub additional_context: Option<String>,
}

/// Response from component generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentGenerationResponse {
    pub component_code: String,
    pub component_name: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub validation_status: ValidationStatus,
}

/// Status of component validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Valid,
    Warning { message: String },
    Error { message: String },
}

/// Streaming event for real-time updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingEvent {
    pub event_type: StreamingEventType,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingEventType {
    Progress,
    PartialResult,
    Complete,
    Error,
}