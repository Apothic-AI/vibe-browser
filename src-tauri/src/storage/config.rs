use crate::ai::AIProviderConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Configuration manager for storing application settings and AI provider configurations
#[derive(Clone)]
pub struct ConfigManager {
    pool: SqlitePool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIProviderEntry {
    pub id: i64,
    pub provider_type: String,
    pub name: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_provider: Option<String>,
    pub cache_enabled: bool,
    pub cache_ttl_hours: u32,
    pub streaming_enabled: bool,
    pub debug_mode: bool,
    pub auto_cleanup_enabled: bool,
    pub theme: String,
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_provider: None,
            cache_enabled: true,
            cache_ttl_hours: 24,
            streaming_enabled: true,
            debug_mode: false,
            auto_cleanup_enabled: true,
            theme: "system".to_string(),
            language: "en".to_string(),
        }
    }
}

impl ConfigManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Store or update an AI provider configuration
    pub async fn store_ai_provider(&self, config: AIProviderConfig, name: &str) -> Result<i64> {
        let result = sqlx::query(r#"
            INSERT OR REPLACE INTO ai_providers 
            (provider_type, name, api_key, base_url, model, max_tokens, temperature, is_active, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#)
        .bind(&config.provider_type)
        .bind(name)
        .bind(&config.api_key)
        .bind(&config.base_url)
        .bind(&config.model)
        .bind(config.max_tokens.map(|t| t as i64))
        .bind(config.temperature.map(|t| t as f64))
        .bind(false) // Default to inactive
        .execute(&self.pool)
        .await?;

        log::info!("Stored AI provider configuration: {}", name);
        Ok(result.last_insert_rowid())
    }

    /// Get an AI provider configuration by name
    pub async fn get_ai_provider(&self, name: &str) -> Result<Option<AIProviderConfig>> {
        let row = sqlx::query(
            r#"
            SELECT provider_type, api_key, base_url, model, max_tokens, temperature
            FROM ai_providers 
            WHERE name = ?
        "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let max_tokens: Option<i64> = row.get("max_tokens");
            let temperature: Option<f64> = row.get("temperature");

            Ok(Some(AIProviderConfig {
                provider_type: row.get("provider_type"),
                api_key: row.get("api_key"),
                base_url: row.get("base_url"),
                model: row.get("model"),
                max_tokens: max_tokens.map(|t| t as u32),
                temperature: temperature.map(|t| t as f32),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all AI provider configurations
    pub async fn list_ai_providers(&self) -> Result<Vec<AIProviderEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, provider_type, name, api_key, base_url, model, max_tokens, 
                   temperature, is_active, created_at, updated_at
            FROM ai_providers 
            ORDER BY updated_at DESC
        "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut providers = Vec::new();
        for row in rows {
            let max_tokens: Option<i64> = row.get("max_tokens");
            let temperature: Option<f64> = row.get("temperature");
            let created_at_str: String = row.get("created_at");
            let updated_at_str: String = row.get("updated_at");

            let created_at = chrono::DateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let updated_at = chrono::DateTime::parse_from_str(&updated_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            providers.push(AIProviderEntry {
                id: row.get("id"),
                provider_type: row.get("provider_type"),
                name: row.get("name"),
                api_key: row.get("api_key"),
                base_url: row.get("base_url"),
                model: row.get("model"),
                max_tokens: max_tokens.map(|t| t as u32),
                temperature: temperature.map(|t| t as f32),
                is_active: row.get("is_active"),
                created_at,
                updated_at,
            });
        }

        Ok(providers)
    }

    /// Set the active AI provider
    pub async fn set_active_provider(&self, name: &str) -> Result<bool> {
        // First, deactivate all providers
        sqlx::query("UPDATE ai_providers SET is_active = FALSE")
            .execute(&self.pool)
            .await?;

        // Then activate the specified provider
        let result = sqlx::query(
            r#"
            UPDATE ai_providers 
            SET is_active = TRUE, updated_at = CURRENT_TIMESTAMP
            WHERE name = ?
        "#,
        )
        .bind(name)
        .execute(&self.pool)
        .await?;

        let success = result.rows_affected() > 0;
        if success {
            log::info!("Set active AI provider: {}", name);
        }

        Ok(success)
    }

    /// Get the currently active AI provider
    pub async fn get_active_provider(&self) -> Result<Option<AIProviderConfig>> {
        let row = sqlx::query(
            r#"
            SELECT provider_type, api_key, base_url, model, max_tokens, temperature
            FROM ai_providers 
            WHERE is_active = TRUE
            LIMIT 1
        "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let max_tokens: Option<i64> = row.get("max_tokens");
            let temperature: Option<f64> = row.get("temperature");

            Ok(Some(AIProviderConfig {
                provider_type: row.get("provider_type"),
                api_key: row.get("api_key"),
                base_url: row.get("base_url"),
                model: row.get("model"),
                max_tokens: max_tokens.map(|t| t as u32),
                temperature: temperature.map(|t| t as f32),
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete an AI provider configuration
    pub async fn delete_ai_provider(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM ai_providers WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        let success = result.rows_affected() > 0;
        if success {
            log::info!("Deleted AI provider: {}", name);
        }

        Ok(success)
    }

    /// Store application configuration
    pub async fn store_app_config(&self, config: &AppConfig) -> Result<()> {
        let config_json = serde_json::to_string(config)?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO configuration (key, value, updated_at)
            VALUES ('app_config', ?, CURRENT_TIMESTAMP)
        "#,
        )
        .bind(&config_json)
        .execute(&self.pool)
        .await?;

        log::info!("Stored application configuration");
        Ok(())
    }

    /// Get application configuration
    pub async fn get_app_config(&self) -> Result<AppConfig> {
        let row = sqlx::query("SELECT value FROM configuration WHERE key = 'app_config'")
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let config_json: String = row.get("value");
            let config: AppConfig = serde_json::from_str(&config_json).unwrap_or_default();
            Ok(config)
        } else {
            // Return default configuration if not found
            let default_config = AppConfig::default();
            self.store_app_config(&default_config).await?;
            Ok(default_config)
        }
    }

    /// Store a generic configuration value
    pub async fn set_config_value(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO configuration (key, value, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
        "#,
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a generic configuration value
    pub async fn get_config_value(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM configuration WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("value")))
    }

    /// Get all configuration values
    pub async fn get_all_config(&self) -> Result<HashMap<String, String>> {
        let rows = sqlx::query("SELECT key, value FROM configuration")
            .fetch_all(&self.pool)
            .await?;

        let mut config = HashMap::new();
        for row in rows {
            let key: String = row.get("key");
            let value: String = row.get("value");
            config.insert(key, value);
        }

        Ok(config)
    }

    /// Delete a configuration value
    pub async fn delete_config_value(&self, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM configuration WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Validate an AI provider configuration
    pub async fn validate_ai_provider(&self, config: &AIProviderConfig) -> Result<bool> {
        // Basic validation
        if config.api_key.is_empty() {
            return Ok(false);
        }

        if config.model.is_empty() {
            return Ok(false);
        }

        // Provider-specific validation
        match config.provider_type.as_str() {
            "openrouter" => {
                // OpenRouter validation
                if !config.api_key.starts_with("sk-") {
                    return Ok(false);
                }
            }
            "vertex" => {
                // Vertex AI validation
                // Could validate project ID format, etc.
            }
            _ => {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
