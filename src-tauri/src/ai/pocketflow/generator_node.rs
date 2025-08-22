use super::super::{WorkflowNode, ComponentGenerationResponse, ValidationStatus};
use super::super::providers::{AIProvider, CompletionRequest, ConcreteAIProvider};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node responsible for generating component code using AI
pub struct GeneratorNode {
    pub node_id: String,
    ai_provider: ConcreteAIProvider,
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

impl GeneratorNode {
    pub fn new(ai_provider: ConcreteAIProvider) -> Self {
        Self {
            node_id: "component_generator".to_string(),
            ai_provider,
        }
    }

    /// Generate the system prompt for component generation
    fn build_system_prompt(&self) -> String {
        r#"You are an expert SolidJS component developer. Your task is to generate high-quality, production-ready SolidJS components based on user requirements.

GUIDELINES:
1. Use SolidJS syntax and patterns exclusively
2. Follow modern TypeScript best practices
3. Implement proper accessibility (a11y) features
4. Use semantic HTML elements
5. Include proper error handling where applicable
6. Make components reusable and configurable through props
7. Use SolidJS reactivity patterns (createSignal, createMemo, etc.)
8. Include proper JSDoc comments for props and complex logic

RESPONSE FORMAT:
Return a JSON object with the following structure:
{
  "component_code": "// Complete SolidJS component code here",
  "component_name": "ComponentName",
  "description": "Brief description of what the component does",
  "dependencies": ["solid-js", "other-deps"]
}

The component_code should be complete and ready to use, including all imports and exports."#.to_string()
    }

    /// Generate the user prompt based on processed requirements
    fn build_user_prompt(&self, requirements: &ProcessedRequirements) -> String {
        format!(
            r#"Generate a SolidJS component with the following specifications:

COMPONENT TYPE: {}

FUNCTIONALITY REQUIREMENTS:
{}

STYLING REQUIREMENTS:
{}

DEPENDENCIES TO CONSIDER:
{}

COMPLEXITY LEVEL: {:.1}/5.0

Please generate a complete, working SolidJS component that fulfills these requirements. 
The component should be:
- Type-safe with proper TypeScript interfaces
- Accessible with proper ARIA attributes
- Well-documented with JSDoc comments
- Modular and reusable
- Following SolidJS best practices

Ensure the component is production-ready and includes proper error handling where appropriate."#,
            requirements.component_type,
            requirements.functionality.join("\n- "),
            requirements.styling_requirements.join("\n- "),
            requirements.dependencies.join(", "),
            requirements.complexity_score
        )
    }

    /// Parse the AI response and extract component information
    fn parse_ai_response(&self, response_content: &str) -> Result<ComponentGenerationResponse> {
        // Try to parse as JSON first
        if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(response_content) {
            let component_code = json_response.get("component_code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let component_name = json_response.get("component_name")
                .and_then(|v| v.as_str())
                .unwrap_or("GeneratedComponent")
                .to_string();
            
            let description = json_response.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Generated SolidJS component")
                .to_string();
            
            let dependencies = json_response.get("dependencies")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect())
                .unwrap_or_else(|| vec!["solid-js".to_string()]);

            return Ok(ComponentGenerationResponse {
                component_code,
                component_name,
                description,
                dependencies,
                validation_status: ValidationStatus::Valid,
            });
        }

        // Fallback: try to extract component code from markdown or plain text
        let component_code = self.extract_component_from_text(response_content);
        let component_name = self.extract_component_name(&component_code);

        Ok(ComponentGenerationResponse {
            component_code,
            component_name,
            description: "Generated SolidJS component".to_string(),
            dependencies: vec!["solid-js".to_string()],
            validation_status: ValidationStatus::Warning {
                message: "Generated from non-JSON response, may need manual review".to_string(),
            },
        })
    }

    /// Extract component code from text (handles markdown code blocks)
    fn extract_component_from_text(&self, text: &str) -> String {
        // Look for TypeScript/JSX code blocks
        let patterns = [
            r"```(?:tsx|typescript|jsx|javascript)\n(.*?)\n```",
            r"```\n(.*?)\n```",
        ];

        for pattern in &patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if let Some(captures) = regex.captures(text) {
                    if let Some(code) = captures.get(1) {
                        return code.as_str().to_string();
                    }
                }
            }
        }

        // If no code blocks found, return the whole text
        text.to_string()
    }

    /// Extract component name from code
    fn extract_component_name(&self, code: &str) -> String {
        // Look for export default function ComponentName
        if let Ok(regex) = regex::Regex::new(r"export\s+default\s+function\s+(\w+)") {
            if let Some(captures) = regex.captures(code) {
                if let Some(name) = captures.get(1) {
                    return name.as_str().to_string();
                }
            }
        }

        // Look for function ComponentName
        if let Ok(regex) = regex::Regex::new(r"function\s+(\w+)") {
            if let Some(captures) = regex.captures(code) {
                if let Some(name) = captures.get(1) {
                    return name.as_str().to_string();
                }
            }
        }

        // Look for const ComponentName = 
        if let Ok(regex) = regex::Regex::new(r"const\s+(\w+)\s*=") {
            if let Some(captures) = regex.captures(code) {
                if let Some(name) = captures.get(1) {
                    return name.as_str().to_string();
                }
            }
        }

        "GeneratedComponent".to_string()
    }

    /// Generate component using AI provider
    async fn generate_component(&self, requirements: &ProcessedRequirements) -> Result<ComponentGenerationResponse> {
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(requirements);

        let completion_request = CompletionRequest {
            prompt: user_prompt,
            system_prompt: Some(system_prompt),
            max_tokens: Some(requirements.estimated_tokens.max(500)),
            temperature: Some(0.3), // Lower temperature for more consistent code generation
            stop_sequences: None,
        };

        let response = self.ai_provider.complete(completion_request).await?;
        self.parse_ai_response(&response.content)
    }
}

#[async_trait]
impl WorkflowNode for GeneratorNode {
    async fn execute(&self, inputs: HashMap<String, serde_json::Value>) -> Result<HashMap<String, serde_json::Value>> {
        let requirements: ProcessedRequirements = serde_json::from_value(
            inputs.get("processed_requirements")
                .ok_or_else(|| anyhow::anyhow!("Missing 'processed_requirements' input"))?
                .clone()
        )?;

        let generated_component = self.generate_component(&requirements).await?;
        
        let mut outputs = HashMap::new();
        outputs.insert("generated_component".to_string(), serde_json::to_value(&generated_component)?);
        
        // Pass through previous inputs for the validation node
        if let Some(original_request) = inputs.get("original_request") {
            outputs.insert("original_request".to_string(), original_request.clone());
        }
        outputs.insert("processed_requirements".to_string(), serde_json::to_value(&requirements)?);
        
        Ok(outputs)
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }

    fn node_type(&self) -> &str {
        "component_generator"
    }

    fn validate_inputs(&self, inputs: &HashMap<String, serde_json::Value>) -> Result<()> {
        if !inputs.contains_key("processed_requirements") {
            return Err(anyhow::anyhow!("Missing required input: processed_requirements"));
        }
        Ok(())
    }
}

// Add regex dependency to Cargo.toml
// We'll need this for pattern matching in the text extraction