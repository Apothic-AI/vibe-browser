pub mod openrouter;
pub mod vertex;

pub use self::AIProviderEnum as ConcreteAIProvider;
pub use openrouter::OpenRouterProvider;
pub use vertex::VertexProvider;

use super::AIProviderConfig;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Request for AI completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

/// Response from AI completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    pub usage: Usage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Trait for AI providers
#[async_trait]
pub trait AIProvider: Send + Sync {
    /// Generate completion for the given request
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Generate streaming completion
    async fn stream_complete<F>(
        &self,
        request: CompletionRequest,
        callback: F,
    ) -> Result<CompletionResponse>
    where
        F: Fn(String) + Send + Sync + 'static;

    /// Get provider name
    fn provider_name(&self) -> &str;

    /// Get available models
    async fn get_models(&self) -> Result<Vec<String>>;

    /// Validate the provider configuration
    async fn validate_config(&self) -> Result<()>;
}

/// Enum wrapper for different AI providers to avoid trait object issues
#[derive(Clone)]
pub enum AIProviderEnum {
    OpenRouter(OpenRouterProvider),
    Vertex(VertexProvider),
}

#[async_trait]
impl AIProvider for AIProviderEnum {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        match self {
            AIProviderEnum::OpenRouter(provider) => provider.complete(request).await,
            AIProviderEnum::Vertex(provider) => provider.complete(request).await,
        }
    }

    async fn stream_complete<F>(
        &self,
        request: CompletionRequest,
        callback: F,
    ) -> Result<CompletionResponse>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        match self {
            AIProviderEnum::OpenRouter(provider) => {
                provider.stream_complete(request, callback).await
            }
            AIProviderEnum::Vertex(provider) => provider.stream_complete(request, callback).await,
        }
    }

    fn provider_name(&self) -> &str {
        match self {
            AIProviderEnum::OpenRouter(provider) => provider.provider_name(),
            AIProviderEnum::Vertex(provider) => provider.provider_name(),
        }
    }

    async fn get_models(&self) -> Result<Vec<String>> {
        match self {
            AIProviderEnum::OpenRouter(provider) => provider.get_models().await,
            AIProviderEnum::Vertex(provider) => provider.get_models().await,
        }
    }

    async fn validate_config(&self) -> Result<()> {
        match self {
            AIProviderEnum::OpenRouter(provider) => provider.validate_config().await,
            AIProviderEnum::Vertex(provider) => provider.validate_config().await,
        }
    }
}

/// Factory for creating AI providers
pub struct AIProviderFactory;

impl AIProviderFactory {
    pub fn create_provider(config: AIProviderConfig) -> Result<AIProviderEnum> {
        match config.provider_type.as_str() {
            "openrouter" => {
                let provider = OpenRouterProvider::new(config)?;
                Ok(AIProviderEnum::OpenRouter(provider))
            }
            "vertex" => {
                let provider = VertexProvider::new(config)?;
                Ok(AIProviderEnum::Vertex(provider))
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported provider type: {}",
                config.provider_type
            )),
        }
    }

    pub fn get_supported_providers() -> Vec<&'static str> {
        vec!["openrouter", "vertex"]
    }
}
