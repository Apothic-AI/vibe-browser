use crate::ai::ComponentGenerationResponse;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Component cache manager for storing and retrieving generated components
#[derive(Clone)]
pub struct ComponentCache {
    pool: SqlitePool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedComponent {
    pub id: i64,
    pub component_name: String,
    pub requirements_hash: String,
    pub component_code: String,
    pub description: Option<String>,
    pub dependencies: Vec<String>,
    pub validation_status: String, // JSON string
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

impl ComponentCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Generate a hash for the requirements to use as cache key
    fn hash_requirements(
        &self,
        requirements: &str,
        component_type: &Option<String>,
        style_framework: &Option<String>,
    ) -> String {
        let mut hasher = DefaultHasher::new();
        requirements.hash(&mut hasher);
        component_type.hash(&mut hasher);
        style_framework.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Store a component in the cache
    pub async fn store_component(
        &self,
        requirements: &str,
        component_type: &Option<String>,
        style_framework: &Option<String>,
        response: &ComponentGenerationResponse,
    ) -> Result<i64> {
        let requirements_hash =
            self.hash_requirements(requirements, component_type, style_framework);
        let dependencies_json = serde_json::to_string(&response.dependencies)?;
        let validation_status_json = serde_json::to_string(&response.validation_status)?;

        let result = sqlx::query(r#"
            INSERT OR REPLACE INTO component_cache 
            (component_name, requirements_hash, component_code, description, dependencies, validation_status)
            VALUES (?, ?, ?, ?, ?, ?)
        "#)
        .bind(&response.component_name)
        .bind(&requirements_hash)
        .bind(&response.component_code)
        .bind(&response.description)
        .bind(&dependencies_json)
        .bind(&validation_status_json)
        .execute(&self.pool)
        .await?;

        log::info!(
            "Cached component '{}' with hash '{}'",
            response.component_name,
            requirements_hash
        );
        Ok(result.last_insert_rowid())
    }

    /// Retrieve a component from the cache
    pub async fn get_component(
        &self,
        requirements: &str,
        component_type: &Option<String>,
        style_framework: &Option<String>,
    ) -> Result<Option<ComponentGenerationResponse>> {
        let requirements_hash =
            self.hash_requirements(requirements, component_type, style_framework);

        let row = sqlx::query(
            r#"
            SELECT component_name, component_code, description, dependencies, validation_status
            FROM component_cache 
            WHERE requirements_hash = ?
        "#,
        )
        .bind(&requirements_hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            // Update last accessed time
            self.update_last_accessed(&requirements_hash).await?;

            let component_name: String = row.get("component_name");
            let component_code: String = row.get("component_code");
            let description: String = row.get("description");
            let dependencies_json: String = row.get("dependencies");
            let validation_status_json: String = row.get("validation_status");

            let dependencies: Vec<String> = serde_json::from_str(&dependencies_json)?;
            let validation_status = serde_json::from_str(&validation_status_json)?;

            log::info!(
                "Retrieved cached component '{}' with hash '{}'",
                component_name,
                requirements_hash
            );

            Ok(Some(ComponentGenerationResponse {
                component_name,
                component_code,
                description,
                dependencies,
                validation_status,
            }))
        } else {
            log::debug!("No cached component found for hash '{}'", requirements_hash);
            Ok(None)
        }
    }

    /// Update the last accessed time for a cached component
    async fn update_last_accessed(&self, requirements_hash: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE component_cache 
            SET last_accessed = CURRENT_TIMESTAMP 
            WHERE requirements_hash = ?
        "#,
        )
        .bind(requirements_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all cached components with pagination
    pub async fn list_cached_components(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CachedComponent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, component_name, requirements_hash, component_code, description, 
                   dependencies, validation_status, created_at, last_accessed
            FROM component_cache 
            ORDER BY last_accessed DESC
            LIMIT ? OFFSET ?
        "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut components = Vec::new();
        for row in rows {
            let dependencies_json: String = row.get("dependencies");
            let dependencies: Vec<String> =
                serde_json::from_str(&dependencies_json).unwrap_or_else(|_| vec![]);

            let created_at_str: String = row.get("created_at");
            let last_accessed_str: String = row.get("last_accessed");

            // Parse SQLite datetime strings
            let created_at = chrono::DateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let last_accessed =
                chrono::DateTime::parse_from_str(&last_accessed_str, "%Y-%m-%d %H:%M:%S")
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

            components.push(CachedComponent {
                id: row.get("id"),
                component_name: row.get("component_name"),
                requirements_hash: row.get("requirements_hash"),
                component_code: row.get("component_code"),
                description: row.get("description"),
                dependencies,
                validation_status: row.get("validation_status"),
                created_at,
                last_accessed,
            });
        }

        Ok(components)
    }

    /// Delete a cached component by ID
    pub async fn delete_component(&self, id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM component_cache WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete a cached component by requirements hash
    pub async fn delete_component_by_hash(&self, requirements_hash: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM component_cache WHERE requirements_hash = ?")
            .bind(requirements_hash)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Clear all cached components
    pub async fn clear_cache(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM component_cache")
            .execute(&self.pool)
            .await?;

        log::info!("Cleared {} cached components", result.rows_affected());
        Ok(result.rows_affected())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let total_components: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM component_cache")
            .fetch_one(&self.pool)
            .await?;

        let total_size_bytes: i64 =
            sqlx::query_scalar("SELECT SUM(LENGTH(component_code)) FROM component_cache")
                .fetch_one(&self.pool)
                .await?;

        let most_recent_row =
            sqlx::query("SELECT created_at FROM component_cache ORDER BY created_at DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;

        let last_update = if let Some(row) = most_recent_row {
            let created_at_str: String = row.get("created_at");
            chrono::DateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        } else {
            None
        };

        Ok(CacheStats {
            total_components: total_components as u64,
            total_size_bytes: total_size_bytes as u64,
            average_component_size: if total_components > 0 {
                (total_size_bytes as f64) / (total_components as f64)
            } else {
                0.0
            },
            last_update,
        })
    }

    /// Search for cached components by name or content
    pub async fn search_components(&self, query: &str, limit: i64) -> Result<Vec<CachedComponent>> {
        let search_pattern = format!("%{}%", query);

        let rows = sqlx::query(
            r#"
            SELECT id, component_name, requirements_hash, component_code, description, 
                   dependencies, validation_status, created_at, last_accessed
            FROM component_cache 
            WHERE component_name LIKE ? OR description LIKE ? OR component_code LIKE ?
            ORDER BY last_accessed DESC
            LIMIT ?
        "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut components = Vec::new();
        for row in rows {
            let dependencies_json: String = row.get("dependencies");
            let dependencies: Vec<String> =
                serde_json::from_str(&dependencies_json).unwrap_or_else(|_| vec![]);

            let created_at_str: String = row.get("created_at");
            let last_accessed_str: String = row.get("last_accessed");

            let created_at = chrono::DateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let last_accessed =
                chrono::DateTime::parse_from_str(&last_accessed_str, "%Y-%m-%d %H:%M:%S")
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

            components.push(CachedComponent {
                id: row.get("id"),
                component_name: row.get("component_name"),
                requirements_hash: row.get("requirements_hash"),
                component_code: row.get("component_code"),
                description: row.get("description"),
                dependencies,
                validation_status: row.get("validation_status"),
                created_at,
                last_accessed,
            });
        }

        Ok(components)
    }
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_components: u64,
    pub total_size_bytes: u64,
    pub average_component_size: f64,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
}
