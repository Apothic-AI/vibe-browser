pub mod generator_node;
pub mod requirements_node;
pub mod validation_node;

pub use generator_node::GeneratorNode;
pub use requirements_node::RequirementsNode;
pub use validation_node::ValidationNode;

use super::{ComponentGenerationRequest, ComponentGenerationResponse, WorkflowNode};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Orchestrates the complete component generation workflow
pub struct PocketFlowOrchestrator {
    requirements_node: Arc<RequirementsNode>,
    generator_node: Arc<GeneratorNode>,
    validation_node: Arc<ValidationNode>,
}

impl PocketFlowOrchestrator {
    pub fn new(
        requirements_node: RequirementsNode,
        generator_node: GeneratorNode,
        validation_node: ValidationNode,
    ) -> Self {
        Self {
            requirements_node: Arc::new(requirements_node),
            generator_node: Arc::new(generator_node),
            validation_node: Arc::new(validation_node),
        }
    }

    /// Execute the complete workflow for component generation
    pub async fn generate_component(
        &self,
        request: ComponentGenerationRequest,
    ) -> Result<ComponentGenerationResponse> {
        // Step 1: Process requirements
        let mut inputs = HashMap::new();
        inputs.insert("request".to_string(), serde_json::to_value(&request)?);

        let requirements_output = self.requirements_node.execute(inputs).await?;

        // Step 2: Generate component
        let generation_output = self.generator_node.execute(requirements_output).await?;

        // Step 3: Validate component
        let validation_output = self.validation_node.execute(generation_output).await?;

        // Extract final response
        let response: ComponentGenerationResponse = serde_json::from_value(
            validation_output
                .get("response")
                .ok_or_else(|| anyhow::anyhow!("Missing response in validation output"))?
                .clone(),
        )?;

        Ok(response)
    }

    /// Execute workflow with streaming updates
    pub async fn generate_component_streaming<F>(
        &self,
        request: ComponentGenerationRequest,
        mut callback: F,
    ) -> Result<ComponentGenerationResponse>
    where
        F: FnMut(super::StreamingEvent) + Send + 'static,
    {
        use super::{StreamingEvent, StreamingEventType};

        // Step 1: Requirements processing
        callback(StreamingEvent {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({"stage": "requirements", "progress": 0.1}),
            timestamp: chrono::Utc::now(),
        });

        let mut inputs = HashMap::new();
        inputs.insert("request".to_string(), serde_json::to_value(&request)?);

        let requirements_output = self.requirements_node.execute(inputs).await?;

        callback(StreamingEvent {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({"stage": "requirements", "progress": 0.3}),
            timestamp: chrono::Utc::now(),
        });

        // Step 2: Component generation
        callback(StreamingEvent {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({"stage": "generation", "progress": 0.4}),
            timestamp: chrono::Utc::now(),
        });

        let generation_output = self.generator_node.execute(requirements_output).await?;

        callback(StreamingEvent {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({"stage": "generation", "progress": 0.7}),
            timestamp: chrono::Utc::now(),
        });

        // Step 3: Validation
        callback(StreamingEvent {
            event_type: StreamingEventType::Progress,
            data: serde_json::json!({"stage": "validation", "progress": 0.8}),
            timestamp: chrono::Utc::now(),
        });

        let validation_output = self.validation_node.execute(generation_output).await?;

        let response: ComponentGenerationResponse = serde_json::from_value(
            validation_output
                .get("response")
                .ok_or_else(|| anyhow::anyhow!("Missing response in validation output"))?
                .clone(),
        )?;

        callback(StreamingEvent {
            event_type: StreamingEventType::Complete,
            data: serde_json::to_value(&response)?,
            timestamp: chrono::Utc::now(),
        });

        Ok(response)
    }
}
