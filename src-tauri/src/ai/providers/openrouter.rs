use super::{AIProvider, CompletionRequest, CompletionResponse, Usage};
use crate::ai::AIProviderConfig;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    stop: Option<Vec<String>>,
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    usage: Option<OpenRouterUsage>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    description: Option<String>,
}

impl OpenRouterProvider {
    pub fn new(config: AIProviderConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        let base_url = config.base_url
            .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());

        Ok(Self {
            client,
            api_key: config.api_key,
            base_url,
            model: config.model,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    fn build_messages(&self, request: &CompletionRequest) -> Vec<OpenRouterMessage> {
        let mut messages = Vec::new();

        if let Some(ref system_prompt) = request.system_prompt {
            messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: system_prompt.clone(),
            });
        }

        messages.push(OpenRouterMessage {
            role: "user".to_string(),
            content: request.prompt.clone(),
        });

        messages
    }

    fn convert_usage(&self, usage: Option<OpenRouterUsage>) -> Usage {
        match usage {
            Some(u) => Usage {
                prompt_tokens: u.prompt_tokens.unwrap_or(0),
                completion_tokens: u.completion_tokens.unwrap_or(0),
                total_tokens: u.total_tokens.unwrap_or(0),
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
impl AIProvider for OpenRouterProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let messages = self.build_messages(&request);
        
        let openrouter_request = OpenRouterRequest {
            model: self.model.clone(),
            messages,
            max_tokens: request.max_tokens.or(self.max_tokens),
            temperature: request.temperature.or(self.temperature),
            stop: request.stop_sequences,
            stream: Some(false),
        };

        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/Apothic-AI/vibe-browser")
            .header("X-Title", "Vibe Browser")
            .json(&openrouter_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("OpenRouter API error: {}", error_text));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;
        
        let choice = openrouter_response.choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No choices in OpenRouter response"))?;

        Ok(CompletionResponse {
            content: choice.message.content,
            model: openrouter_response.model.unwrap_or_else(|| self.model.clone()),
            usage: self.convert_usage(openrouter_response.usage),
            finish_reason: choice.finish_reason,
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
        // TODO: Implement actual streaming with Server-Sent Events
        let response = self.complete(request).await?;
        callback(response.content.clone());
        Ok(response)
    }

    fn provider_name(&self) -> &str {
        "openrouter"
    }

    async fn get_models(&self) -> Result<Vec<String>> {
        let response = self.client
            .get(&format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch models from OpenRouter"));
        }

        let models_response: OpenRouterModelsResponse = response.json().await?;
        Ok(models_response.data.into_iter().map(|m| m.id).collect())
    }

    async fn validate_config(&self) -> Result<()> {
        // Try to fetch models to validate the API key
        let _models = self.get_models().await?;
        Ok(())
    }
}