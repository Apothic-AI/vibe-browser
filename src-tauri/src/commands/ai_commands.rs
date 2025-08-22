use crate::ai::{
    AIProviderConfig, ComponentGenerationRequest, ComponentGenerationResponse, WorkflowNode,
    pocketflow::{PocketFlowOrchestrator, RequirementsNode, GeneratorNode, ValidationNode},
    providers::{AIProviderFactory, AIProvider},
    streaming::{StreamingManager},
};
use crate::storage::{ComponentCache, ConfigManager};
use crate::commands::{CommandResponse, PaginationParams, SearchParams};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// State for AI components
#[derive(Clone)]
pub struct AIState {
    pub orchestrator: Option<Arc<PocketFlowOrchestrator>>,
    pub cache: ComponentCache,
    pub config_manager: ConfigManager,
    pub streaming_manager: StreamingManager,
}

impl AIState {
    pub fn new(
        cache: ComponentCache,
        config_manager: ConfigManager,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            orchestrator: None,
            cache,
            config_manager,
            streaming_manager: StreamingManager::new(app_handle),
        }
    }

    /// Initialize the orchestrator with the active AI provider
    pub async fn initialize_orchestrator(&mut self) -> Result<()> {
        if let Some(provider_config) = self.config_manager.get_active_provider().await? {
            let provider = AIProviderFactory::create_provider(provider_config)?;
            
            let requirements_node = RequirementsNode::new();
            let generator_node = GeneratorNode::new(provider);
            let validation_node = ValidationNode::new();
            
            let orchestrator = PocketFlowOrchestrator::new(
                requirements_node,
                generator_node,
                validation_node,
            );
            
            self.orchestrator = Some(Arc::new(orchestrator));
            log::info!("AI orchestrator initialized successfully");
        }
        
        Ok(())
    }
}

/// Generate a single component
#[tauri::command]
pub async fn generate_component(
    request: ComponentGenerationRequest,
    state: State<'_, AIState>,
) -> Result<CommandResponse<ComponentGenerationResponse>, String> {
    log::info!("Generating component for requirements: {}", request.requirements);

    // Check cache first
    if let Ok(Some(cached_response)) = state.cache.get_component(
        &request.requirements,
        &request.component_type,
        &request.style_framework,
    ).await {
        log::info!("Returning cached component: {}", cached_response.component_name);
        return Ok(CommandResponse::success(cached_response));
    }

    // Ensure orchestrator is initialized
    if state.orchestrator.is_none() {
        return Ok(CommandResponse::error(
            "AI provider not configured. Please configure an AI provider first.".to_string()
        ));
    }

    let orchestrator = state.orchestrator.as_ref().unwrap();
    
    match orchestrator.generate_component(request.clone()).await {
        Ok(response) => {
            // Cache the successful response
            if let Err(e) = state.cache.store_component(
                &request.requirements,
                &request.component_type,
                &request.style_framework,
                &response,
            ).await {
                log::warn!("Failed to cache component: {}", e);
            }

            log::info!("Component generated successfully: {}", response.component_name);
            Ok(CommandResponse::success(response))
        }
        Err(e) => {
            log::error!("Failed to generate component: {}", e);
            Ok(CommandResponse::error(format!("Failed to generate component: {}", e)))
        }
    }
}

/// Generate component with real-time streaming updates
#[tauri::command]
pub async fn stream_component_generation(
    request: ComponentGenerationRequest,
    session_id: String,
    state: State<'_, AIState>,
) -> Result<CommandResponse<String>, String> {
    log::info!("Starting streaming component generation for session: {}", session_id);

    // Check cache first
    if let Ok(Some(cached_response)) = state.cache.get_component(
        &request.requirements,
        &request.component_type,
        &request.style_framework,
    ).await {
        // Send cached result immediately
        let event = crate::ai::streaming::utils::completion_event(&cached_response);
        if let Err(e) = state.streaming_manager.emit_event(event) {
            log::error!("Failed to emit cached result: {}", e);
        }
        return Ok(CommandResponse::success("Cached result sent".to_string()));
    }

    // Ensure orchestrator is initialized
    if state.orchestrator.is_none() {
        let error_event = crate::ai::streaming::utils::error_event(
            "AI provider not configured",
            Some("Please configure an AI provider first")
        );
        if let Err(e) = state.streaming_manager.emit_event(error_event) {
            log::error!("Failed to emit error event: {}", e);
        }
        return Ok(CommandResponse::error("AI provider not configured".to_string()));
    }

    let orchestrator = state.orchestrator.as_ref().unwrap().clone();
    let cache = state.cache.clone();
    let callback = state.streaming_manager.create_callback();
    
    // Run generation in background task
    tokio::spawn(async move {
        match orchestrator.generate_component_streaming(request.clone(), callback).await {
            Ok(response) => {
                // Cache the successful response
                if let Err(e) = cache.store_component(
                    &request.requirements,
                    &request.component_type,
                    &request.style_framework,
                    &response,
                ).await {
                    log::warn!("Failed to cache streamed component: {}", e);
                }
                log::info!("Streaming component generation completed for session: {}", session_id);
            }
            Err(e) => {
                log::error!("Streaming component generation failed for session {}: {}", session_id, e);
            }
        }
    });

    Ok(CommandResponse::success("Streaming started".to_string()))
}

/// Validate component code
#[tauri::command]
pub async fn validate_component(
    component_code: String,
) -> Result<CommandResponse<ValidationResult>, String> {
    log::info!("Validating component code (length: {} chars)", component_code.len());

    let validation_node = ValidationNode::new();
    
    // Create a mock component response for validation
    let mock_response = ComponentGenerationResponse {
        component_code,
        component_name: "ValidationComponent".to_string(),
        description: "Component for validation".to_string(),
        dependencies: vec!["solid-js".to_string()],
        validation_status: crate::ai::ValidationStatus::Valid,
    };

    let mut inputs = std::collections::HashMap::new();
    inputs.insert("generated_component".to_string(), serde_json::to_value(&mock_response).unwrap());

    match validation_node.execute(inputs).await {
        Ok(outputs) => {
            if let Some(response_value) = outputs.get("response") {
                if let Ok(validated_response) = serde_json::from_value::<ComponentGenerationResponse>(response_value.clone()) {
                    let result = ValidationResult {
                        is_valid: matches!(validated_response.validation_status, crate::ai::ValidationStatus::Valid),
                        status: validated_response.validation_status,
                        suggestions: vec![], // Could be extracted from validation details
                    };
                    return Ok(CommandResponse::success(result));
                }
            }
            Ok(CommandResponse::error("Failed to parse validation result".to_string()))
        }
        Err(e) => {
            log::error!("Component validation failed: {}", e);
            Ok(CommandResponse::error(format!("Validation failed: {}", e)))
        }
    }
}

/// Configure AI provider
#[tauri::command]
pub async fn configure_ai_provider(
    name: String,
    config: AIProviderConfig,
    state: State<'_, AIState>,
) -> Result<CommandResponse<String>, String> {
    log::info!("Configuring AI provider: {} ({})", name, config.provider_type);

    // Validate the configuration
    if !state.config_manager.validate_ai_provider(&config).await.unwrap_or(false) {
        return Ok(CommandResponse::error("Invalid AI provider configuration".to_string()));
    }

    match state.config_manager.store_ai_provider(config, &name).await {
        Ok(_) => {
            // Test the provider by creating it
            match state.config_manager.get_ai_provider(&name).await {
                Ok(Some(stored_config)) => {
                    match AIProviderFactory::create_provider(stored_config) {
                        Ok(provider) => {
                            // Test the provider
                            match provider.validate_config().await {
                                Ok(_) => {
                                    log::info!("AI provider {} configured and validated successfully", name);
                                    Ok(CommandResponse::success("Provider configured successfully".to_string()))
                                }
                                Err(e) => {
                                    log::error!("AI provider validation failed: {}", e);
                                    Ok(CommandResponse::error(format!("Provider validation failed: {}", e)))
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create AI provider: {}", e);
                            Ok(CommandResponse::error(format!("Failed to create provider: {}", e)))
                        }
                    }
                }
                Ok(None) => {
                    Ok(CommandResponse::error("Failed to retrieve stored configuration".to_string()))
                }
                Err(e) => {
                    log::error!("Failed to retrieve AI provider config: {}", e);
                    Ok(CommandResponse::error(format!("Failed to retrieve config: {}", e)))
                }
            }
        }
        Err(e) => {
            log::error!("Failed to store AI provider config: {}", e);
            Ok(CommandResponse::error(format!("Failed to store configuration: {}", e)))
        }
    }
}

/// Set active AI provider
#[tauri::command]
pub async fn set_active_ai_provider(
    name: String,
    state: State<'_, AIState>,
) -> Result<CommandResponse<String>, String> {
    log::info!("Setting active AI provider: {}", name);

    match state.config_manager.set_active_provider(&name).await {
        Ok(true) => {
            // Reinitialize the orchestrator with the new provider
            let mut state_mut = state.inner().clone();
            if let Err(e) = state_mut.initialize_orchestrator().await {
                log::error!("Failed to reinitialize orchestrator: {}", e);
                return Ok(CommandResponse::error(format!("Failed to initialize with new provider: {}", e)));
            }

            log::info!("Active AI provider set to: {}", name);
            Ok(CommandResponse::success("Active provider updated".to_string()))
        }
        Ok(false) => {
            Ok(CommandResponse::error("Provider not found".to_string()))
        }
        Err(e) => {
            log::error!("Failed to set active AI provider: {}", e);
            Ok(CommandResponse::error(format!("Failed to set active provider: {}", e)))
        }
    }
}

/// Get available AI providers
#[tauri::command]
pub async fn get_ai_providers(
    state: State<'_, AIState>,
) -> Result<CommandResponse<Vec<crate::storage::config::AIProviderEntry>>, String> {
    match state.config_manager.list_ai_providers().await {
        Ok(providers) => {
            log::info!("Retrieved {} AI providers", providers.len());
            Ok(CommandResponse::success(providers))
        }
        Err(e) => {
            log::error!("Failed to get AI providers: {}", e);
            Ok(CommandResponse::error(format!("Failed to get providers: {}", e)))
        }
    }
}

/// Get supported provider types
#[tauri::command]
pub async fn get_supported_providers() -> Result<CommandResponse<Vec<String>>, String> {
    let providers = AIProviderFactory::get_supported_providers()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    Ok(CommandResponse::success(providers))
}

/// Delete AI provider
#[tauri::command]
pub async fn delete_ai_provider(
    name: String,
    state: State<'_, AIState>,
) -> Result<CommandResponse<String>, String> {
    log::info!("Deleting AI provider: {}", name);

    match state.config_manager.delete_ai_provider(&name).await {
        Ok(true) => {
            log::info!("AI provider {} deleted successfully", name);
            Ok(CommandResponse::success("Provider deleted".to_string()))
        }
        Ok(false) => {
            Ok(CommandResponse::error("Provider not found".to_string()))
        }
        Err(e) => {
            log::error!("Failed to delete AI provider: {}", e);
            Ok(CommandResponse::error(format!("Failed to delete provider: {}", e)))
        }
    }
}

/// Get cached components
#[tauri::command]
pub async fn get_cached_components(
    pagination: PaginationParams,
    state: State<'_, AIState>,
) -> Result<CommandResponse<Vec<crate::storage::cache::CachedComponent>>, String> {
    let limit = pagination.limit.unwrap_or(20);
    let offset = pagination.offset.unwrap_or(0);

    match state.cache.list_cached_components(limit, offset).await {
        Ok(components) => {
            log::info!("Retrieved {} cached components", components.len());
            Ok(CommandResponse::success(components))
        }
        Err(e) => {
            log::error!("Failed to get cached components: {}", e);
            Ok(CommandResponse::error(format!("Failed to get cached components: {}", e)))
        }
    }
}

/// Search cached components
#[tauri::command]
pub async fn search_cached_components(
    search: SearchParams,
    state: State<'_, AIState>,
) -> Result<CommandResponse<Vec<crate::storage::cache::CachedComponent>>, String> {
    let limit = search.limit.unwrap_or(10);

    match state.cache.search_components(&search.query, limit).await {
        Ok(components) => {
            log::info!("Found {} components matching '{}'", components.len(), search.query);
            Ok(CommandResponse::success(components))
        }
        Err(e) => {
            log::error!("Failed to search cached components: {}", e);
            Ok(CommandResponse::error(format!("Failed to search components: {}", e)))
        }
    }
}

/// Clear component cache
#[tauri::command]
pub async fn clear_component_cache(
    state: State<'_, AIState>,
) -> Result<CommandResponse<String>, String> {
    match state.cache.clear_cache().await {
        Ok(count) => {
            log::info!("Cleared {} cached components", count);
            Ok(CommandResponse::success(format!("Cleared {} components", count)))
        }
        Err(e) => {
            log::error!("Failed to clear component cache: {}", e);
            Ok(CommandResponse::error(format!("Failed to clear cache: {}", e)))
        }
    }
}

/// Get cache statistics
#[tauri::command]
pub async fn get_cache_stats(
    state: State<'_, AIState>,
) -> Result<CommandResponse<crate::storage::cache::CacheStats>, String> {
    match state.cache.get_cache_stats().await {
        Ok(stats) => {
            Ok(CommandResponse::success(stats))
        }
        Err(e) => {
            log::error!("Failed to get cache stats: {}", e);
            Ok(CommandResponse::error(format!("Failed to get cache stats: {}", e)))
        }
    }
}

/// Response types for commands
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub status: crate::ai::ValidationStatus,
    pub suggestions: Vec<String>,
}