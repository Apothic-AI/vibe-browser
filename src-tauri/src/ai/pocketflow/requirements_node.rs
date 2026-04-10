use super::super::{ComponentGenerationRequest, WorkflowNode};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node responsible for processing and analyzing requirements
pub struct RequirementsNode {
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProcessedRequirements {
    pub component_type: String,
    pub functionality: Vec<String>,
    pub styling_requirements: Vec<String>,
    pub dependencies: Vec<String>,
    pub complexity_score: f32,
    pub estimated_tokens: u32,
}

impl RequirementsNode {
    pub fn new() -> Self {
        Self {
            node_id: "requirements_processor".to_string(),
        }
    }

    /// Analyze and structure the requirements
    fn process_requirements(
        &self,
        request: &ComponentGenerationRequest,
    ) -> Result<ProcessedRequirements> {
        let requirements_text = &request.requirements.to_lowercase();

        // Detect component type
        let component_type = self.detect_component_type(requirements_text, &request.component_type);

        // Extract functionality requirements
        let functionality = self.extract_functionality(requirements_text);

        // Extract styling requirements
        let styling_requirements =
            self.extract_styling_requirements(requirements_text, &request.style_framework);

        // Determine dependencies
        let dependencies = self.determine_dependencies(&component_type, &functionality);

        // Calculate complexity score
        let complexity_score = self.calculate_complexity(&functionality, &styling_requirements);

        // Estimate token requirements
        let estimated_tokens = self.estimate_tokens(complexity_score);

        Ok(ProcessedRequirements {
            component_type,
            functionality,
            styling_requirements,
            dependencies,
            complexity_score,
            estimated_tokens,
        })
    }

    fn detect_component_type(&self, requirements: &str, explicit_type: &Option<String>) -> String {
        if let Some(ref comp_type) = explicit_type {
            return comp_type.clone();
        }

        // Common component patterns
        if requirements.contains("button") || requirements.contains("click") {
            "Button".to_string()
        } else if requirements.contains("form") || requirements.contains("input") {
            "Form".to_string()
        } else if requirements.contains("modal") || requirements.contains("dialog") {
            "Modal".to_string()
        } else if requirements.contains("card") || requirements.contains("display") {
            "Card".to_string()
        } else if requirements.contains("list") || requirements.contains("items") {
            "List".to_string()
        } else if requirements.contains("nav") || requirements.contains("menu") {
            "Navigation".to_string()
        } else {
            "Component".to_string()
        }
    }

    fn extract_functionality(&self, requirements: &str) -> Vec<String> {
        let mut functionality = Vec::new();

        // Common functionality patterns
        if requirements.contains("click") || requirements.contains("press") {
            functionality.push("Click handling".to_string());
        }
        if requirements.contains("submit") {
            functionality.push("Form submission".to_string());
        }
        if requirements.contains("validate") {
            functionality.push("Input validation".to_string());
        }
        if requirements.contains("animate") || requirements.contains("transition") {
            functionality.push("Animations".to_string());
        }
        if requirements.contains("responsive") {
            functionality.push("Responsive design".to_string());
        }
        if requirements.contains("accessible") || requirements.contains("a11y") {
            functionality.push("Accessibility features".to_string());
        }
        if requirements.contains("state") || requirements.contains("dynamic") {
            functionality.push("State management".to_string());
        }

        if functionality.is_empty() {
            functionality.push("Basic rendering".to_string());
        }

        functionality
    }

    fn extract_styling_requirements(
        &self,
        requirements: &str,
        framework: &Option<String>,
    ) -> Vec<String> {
        let mut styling = Vec::new();

        // Framework-specific styling
        if let Some(ref fw) = framework {
            styling.push(format!("{} styling", fw));
        }

        // Color requirements
        if requirements.contains("color") || requirements.contains("theme") {
            styling.push("Custom colors".to_string());
        }

        // Layout requirements
        if requirements.contains("flex") || requirements.contains("grid") {
            styling.push("Layout system".to_string());
        }

        // Size requirements
        if requirements.contains("size") || requirements.contains("dimension") {
            styling.push("Size variations".to_string());
        }

        if styling.is_empty() {
            styling.push("Default styling".to_string());
        }

        styling
    }

    fn determine_dependencies(
        &self,
        component_type: &str,
        functionality: &[String],
    ) -> Vec<String> {
        let mut deps = Vec::new();

        // Core SolidJS dependencies
        deps.push("solid-js".to_string());

        // Component-specific dependencies
        match component_type {
            "Form" => {
                deps.push("@felte/solid".to_string());
                deps.push("@felte/validator-yup".to_string());
            }
            "Modal" => {
                deps.push("@kobalte/core".to_string());
            }
            "Navigation" => {
                deps.push("@solidjs/router".to_string());
            }
            _ => {}
        }

        // Functionality-specific dependencies
        for func in functionality {
            if func.contains("Animation") {
                deps.push("@motionone/solid".to_string());
            }
            if func.contains("validation") {
                deps.push("yup".to_string());
            }
        }

        deps.sort();
        deps.dedup();
        deps
    }

    fn calculate_complexity(&self, functionality: &[String], styling: &[String]) -> f32 {
        let base_score = 1.0;
        let func_score = functionality.len() as f32 * 0.3;
        let style_score = styling.len() as f32 * 0.2;

        (base_score + func_score + style_score).min(5.0)
    }

    fn estimate_tokens(&self, complexity: f32) -> u32 {
        let base_tokens = 200;
        let complexity_tokens = (complexity * 150.0) as u32;
        base_tokens + complexity_tokens
    }
}

#[async_trait]
impl WorkflowNode for RequirementsNode {
    async fn execute(
        &self,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let request: ComponentGenerationRequest = serde_json::from_value(
            inputs
                .get("request")
                .ok_or_else(|| anyhow::anyhow!("Missing 'request' input"))?
                .clone(),
        )?;

        let processed = self.process_requirements(&request)?;

        let mut outputs = HashMap::new();
        outputs.insert(
            "original_request".to_string(),
            serde_json::to_value(&request)?,
        );
        outputs.insert(
            "processed_requirements".to_string(),
            serde_json::to_value(&processed)?,
        );

        Ok(outputs)
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }

    fn node_type(&self) -> &str {
        "requirements_processor"
    }

    fn validate_inputs(&self, inputs: &HashMap<String, serde_json::Value>) -> Result<()> {
        if !inputs.contains_key("request") {
            return Err(anyhow::anyhow!("Missing required input: request"));
        }
        Ok(())
    }
}

impl Default for RequirementsNode {
    fn default() -> Self {
        Self::new()
    }
}
