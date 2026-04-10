use super::super::{ComponentGenerationResponse, ValidationStatus, WorkflowNode};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node responsible for validating generated component code
pub struct ValidationNode {
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidationResult {
    is_valid: bool,
    warnings: Vec<String>,
    errors: Vec<String>,
    suggestions: Vec<String>,
}

impl ValidationNode {
    pub fn new() -> Self {
        Self {
            node_id: "component_validator".to_string(),
        }
    }

    /// Perform comprehensive validation of the generated component
    fn validate_component(
        &self,
        component: &ComponentGenerationResponse,
    ) -> Result<ValidationResult> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut suggestions = Vec::new();

        // 1. Basic syntax validation
        self.validate_syntax(&component.component_code, &mut errors, &mut warnings);

        // 2. SolidJS specific validation
        self.validate_solidjs_patterns(&component.component_code, &mut errors, &mut warnings);

        // 3. TypeScript validation
        self.validate_typescript(&component.component_code, &mut warnings, &mut suggestions);

        // 4. Accessibility validation
        self.validate_accessibility(&component.component_code, &mut warnings, &mut suggestions);

        // 5. Best practices validation
        self.validate_best_practices(&component.component_code, &mut warnings, &mut suggestions);

        // 6. Security validation
        self.validate_security(&component.component_code, &mut errors, &mut warnings);

        let is_valid = errors.is_empty();

        Ok(ValidationResult {
            is_valid,
            warnings,
            errors,
            suggestions,
        })
    }

    /// Validate basic JavaScript/TypeScript syntax
    fn validate_syntax(&self, code: &str, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        // Check for balanced brackets
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut bracket_count = 0;

        for char in code.chars() {
            match char {
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                '[' => bracket_count += 1,
                ']' => bracket_count -= 1,
                _ => {}
            }
        }

        if brace_count != 0 {
            errors.push("Unbalanced curly braces detected".to_string());
        }
        if paren_count != 0 {
            errors.push("Unbalanced parentheses detected".to_string());
        }
        if bracket_count != 0 {
            errors.push("Unbalanced square brackets detected".to_string());
        }

        // Check for common syntax issues
        if code.contains(";;") {
            warnings.push("Double semicolons found - possible syntax error".to_string());
        }

        if code.contains("{{") && !code.contains("{{}") {
            warnings.push("Possible JSX syntax error with double braces".to_string());
        }
    }

    /// Validate SolidJS specific patterns and imports
    fn validate_solidjs_patterns(
        &self,
        code: &str,
        errors: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) {
        // Check for SolidJS imports
        let has_solid_import =
            code.contains("from \"solid-js\"") || code.contains("from 'solid-js'");

        // Check if SolidJS features are used
        let uses_solid_features = code.contains("createSignal")
            || code.contains("createMemo")
            || code.contains("createEffect")
            || code.contains("Show")
            || code.contains("For")
            || code.contains("Switch");

        if uses_solid_features && !has_solid_import {
            errors.push("Using SolidJS features without importing from 'solid-js'".to_string());
        }

        // Check for proper JSX syntax
        if !code.contains("return") && (code.contains("<") && code.contains(">")) {
            warnings.push(
                "JSX elements found but no return statement - component may not render".to_string(),
            );
        }

        // Check for proper component export
        if !code.contains("export") && !code.contains("function") {
            warnings.push("No export found - component may not be usable".to_string());
        }

        // Check for SolidJS anti-patterns
        if code.contains("useState") {
            errors.push("React's useState found - use SolidJS createSignal instead".to_string());
        }

        if code.contains("useEffect") {
            errors.push("React's useEffect found - use SolidJS createEffect instead".to_string());
        }
    }

    /// Validate TypeScript patterns and types
    fn validate_typescript(
        &self,
        code: &str,
        warnings: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) {
        // Check for props interface
        if code.contains("props") && !code.contains("interface") && !code.contains("type") {
            suggestions
                .push("Consider defining a TypeScript interface for component props".to_string());
        }

        // Check for any type usage
        if code.contains(": any") {
            warnings.push("Using 'any' type - consider using more specific types".to_string());
        }

        // Check for proper type annotations
        if code.contains("function") && !code.contains("):") {
            suggestions.push("Consider adding return type annotations to functions".to_string());
        }
    }

    /// Validate accessibility features
    fn validate_accessibility(
        &self,
        code: &str,
        warnings: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) {
        // Check for interactive elements without proper accessibility
        if (code.contains("<button") || code.contains("<div") && code.contains("onClick"))
            && !code.contains("aria-")
            && !code.contains("role=")
        {
            suggestions
                .push("Consider adding ARIA attributes for better accessibility".to_string());
        }

        // Check for images without alt text
        if code.contains("<img") && !code.contains("alt=") {
            warnings.push("Images should have alt attributes for accessibility".to_string());
        }

        // Check for form inputs without labels
        if code.contains("<input")
            && !code.contains("aria-label")
            && !code.contains("aria-labelledby")
        {
            suggestions.push("Form inputs should have associated labels".to_string());
        }

        // Check for proper heading structure
        if let Ok(regex) = Regex::new(r"<h(\d)") {
            let headings: Vec<i32> = regex
                .captures_iter(code)
                .filter_map(|cap| cap.get(1))
                .filter_map(|m| m.as_str().parse().ok())
                .collect();

            if headings.len() > 1 {
                let mut prev = 0;
                for &level in &headings {
                    if prev != 0 && level > prev + 1 {
                        suggestions
                            .push("Heading levels should not skip (e.g., h1 to h3)".to_string());
                        break;
                    }
                    prev = level;
                }
            }
        }
    }

    /// Validate best practices
    fn validate_best_practices(
        &self,
        code: &str,
        warnings: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) {
        // Check for inline styles (should prefer CSS classes)
        if code.contains("style={{") || code.contains("style=\"") {
            suggestions.push("Consider using CSS classes instead of inline styles".to_string());
        }

        // Check for hardcoded strings (should consider i18n)
        let string_count = code.matches("\"").count() + code.matches("'").count();
        if string_count > 10 {
            suggestions.push("Consider extracting strings for internationalization".to_string());
        }

        // Check for console.log statements
        if code.contains("console.log") {
            warnings.push("Remove console.log statements before production".to_string());
        }

        // Check for proper error handling
        if code.contains("fetch") && !code.contains("catch") && !code.contains("try") {
            suggestions.push("Consider adding error handling for async operations".to_string());
        }

        // Check for JSDoc comments
        if code.contains("interface") && !code.contains("/**") {
            suggestions.push("Consider adding JSDoc comments for better documentation".to_string());
        }
    }

    /// Validate security considerations
    fn validate_security(&self, code: &str, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        // Check for dangerous innerHTML usage
        if code.contains("innerHTML") || code.contains("dangerouslySetInnerHTML") {
            warnings.push("Be careful with innerHTML - potential XSS vulnerability".to_string());
        }

        // Check for eval usage
        if code.contains("eval(") {
            errors.push("Using eval() is a security risk - avoid if possible".to_string());
        }

        // Check for external script inclusion
        if code.contains("<script") && code.contains("src=") {
            warnings
                .push("External scripts can pose security risks - validate sources".to_string());
        }

        // Check for localStorage usage without validation
        if code.contains("localStorage") && !code.contains("JSON.parse") {
            warnings.push("Consider validating data when using localStorage".to_string());
        }
    }

    /// Apply validation results to the component response
    fn apply_validation_results(
        &self,
        mut component: ComponentGenerationResponse,
        validation: ValidationResult,
    ) -> ComponentGenerationResponse {
        component.validation_status = if !validation.is_valid {
            ValidationStatus::Error {
                message: format!("Validation failed: {}", validation.errors.join(", ")),
            }
        } else if !validation.warnings.is_empty() {
            ValidationStatus::Warning {
                message: format!("Validation warnings: {}", validation.warnings.join(", ")),
            }
        } else {
            ValidationStatus::Valid
        };

        component
    }
}

#[async_trait]
impl WorkflowNode for ValidationNode {
    async fn execute(
        &self,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let component: ComponentGenerationResponse = serde_json::from_value(
            inputs
                .get("generated_component")
                .ok_or_else(|| anyhow::anyhow!("Missing 'generated_component' input"))?
                .clone(),
        )?;

        let validation_result = self.validate_component(&component)?;
        let validated_component = self.apply_validation_results(component, validation_result);

        let mut outputs = HashMap::new();
        outputs.insert(
            "response".to_string(),
            serde_json::to_value(&validated_component)?,
        );

        Ok(outputs)
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }

    fn node_type(&self) -> &str {
        "component_validator"
    }

    fn validate_inputs(&self, inputs: &HashMap<String, serde_json::Value>) -> Result<()> {
        if !inputs.contains_key("generated_component") {
            return Err(anyhow::anyhow!(
                "Missing required input: generated_component"
            ));
        }
        Ok(())
    }
}

impl Default for ValidationNode {
    fn default() -> Self {
        Self::new()
    }
}
