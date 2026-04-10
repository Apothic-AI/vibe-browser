use super::{AIProvider, CompletionRequest, CompletionResponse, Usage};
use crate::ai::AIProviderConfig;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VertexProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    project_id: String,
    location: String,
}

#[derive(Debug, Serialize)]
struct VertexRequest {
    instances: Vec<VertexInstance>,
    parameters: VertexParameters,
}

#[derive(Debug, Serialize)]
struct VertexInstance {
    messages: Vec<VertexMessage>,
}

#[derive(Debug, Serialize)]
struct VertexMessage {
    author: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct VertexParameters {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    temperature: Option<f32>,
    #[serde(rename = "topP")]
    top_p: Option<f32>,
    #[serde(rename = "topK")]
    top_k: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct VertexResponse {
    predictions: Vec<VertexPrediction>,
}

#[derive(Debug, Deserialize)]
struct VertexPrediction {
    candidates: Vec<VertexCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<VertexUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct VertexCandidate {
    content: String,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VertexUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<u32>,
}

impl VertexProvider {
    pub fn new(config: AIProviderConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        // Parse project_id and location from base_url or use defaults
        let (project_id, location) = if let Some(ref url) = config.base_url {
            // Extract from URL like: https://us-central1-aiplatform.googleapis.com/v1/projects/PROJECT_ID/locations/us-central1
            let parts: Vec<&str> = url.split('/').collect();
            let project_id = parts
                .iter()
                .position(|&x| x == "projects")
                .and_then(|i| parts.get(i + 1))
                .map_or("default-project", |v| v)
                .to_string();
            let location = parts
                .iter()
                .position(|&x| x == "locations")
                .and_then(|i| parts.get(i + 1))
                .map_or("us-central1", |v| v)
                .to_string();
            (project_id, location)
        } else {
            ("default-project".to_string(), "us-central1".to_string())
        };

        let base_url = config.base_url.unwrap_or_else(|| {
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}",
                location, project_id, location
            )
        });

        Ok(Self {
            client,
            api_key: config.api_key,
            base_url,
            model: config.model,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            project_id,
            location,
        })
    }

    fn build_messages(&self, request: &CompletionRequest) -> Vec<VertexMessage> {
        let mut messages = Vec::new();

        if let Some(ref system_prompt) = request.system_prompt {
            messages.push(VertexMessage {
                author: "system".to_string(),
                content: system_prompt.clone(),
            });
        }

        messages.push(VertexMessage {
            author: "user".to_string(),
            content: request.prompt.clone(),
        });

        messages
    }

    fn convert_usage(&self, usage: Option<VertexUsageMetadata>) -> Usage {
        match usage {
            Some(u) => Usage {
                prompt_tokens: u.prompt_token_count.unwrap_or(0),
                completion_tokens: u.candidates_token_count.unwrap_or(0),
                total_tokens: u.total_token_count.unwrap_or(0),
            },
            None => Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        }
    }
}

#[async_trait]
impl AIProvider for VertexProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let messages = self.build_messages(&request);

        let vertex_request = VertexRequest {
            instances: vec![VertexInstance { messages }],
            parameters: VertexParameters {
                max_output_tokens: request.max_tokens.or(self.max_tokens),
                temperature: request.temperature.or(self.temperature),
                top_p: Some(0.95),
                top_k: Some(40),
            },
        };

        let endpoint = format!(
            "{}/publishers/google/models/{}:predict",
            self.base_url, self.model
        );

        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&vertex_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Vertex AI API error: {}", error_text));
        }

        let vertex_response: VertexResponse = response.json().await?;

        let prediction = vertex_response
            .predictions
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No predictions in Vertex AI response"))?;

        let candidate = prediction
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No candidates in Vertex AI prediction"))?;

        Ok(CompletionResponse {
            content: candidate.content,
            model: self.model.clone(),
            usage: self.convert_usage(prediction.usage_metadata),
            finish_reason: candidate.finish_reason,
        })
    }

    async fn stream_complete<F>(
        &self,
        request: CompletionRequest,
        callback: F,
    ) -> Result<CompletionResponse>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        // For now, implement as non-streaming and call callback with full response
        // TODO: Implement actual streaming
        let response = self.complete(request).await?;
        callback(response.content.clone());
        Ok(response)
    }

    fn provider_name(&self) -> &str {
        "vertex"
    }

    async fn get_models(&self) -> Result<Vec<String>> {
        // Return common Vertex AI models
        Ok(vec![
            "gemini-1.5-pro-001".to_string(),
            "gemini-1.5-flash-001".to_string(),
            "gemini-1.0-pro-001".to_string(),
            "text-bison@001".to_string(),
            "code-bison@001".to_string(),
        ])
    }

    async fn validate_config(&self) -> Result<()> {
        // For Vertex AI, we could make a simple prediction request to validate
        // For now, just validate that required fields are present
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is required for Vertex AI"));
        }
        if self.project_id.is_empty() {
            return Err(anyhow::anyhow!("Project ID is required for Vertex AI"));
        }
        Ok(())
    }
}
