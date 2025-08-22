pub mod cache;
pub mod config;

pub use cache::ComponentCache;
pub use config::ConfigManager;

use anyhow::Result;
use sqlx::{SqlitePool, Row};
use std::path::PathBuf;

/// Initialize the application database
pub async fn initialize_database(data_dir: &PathBuf) -> Result<SqlitePool> {
    // Ensure the data directory exists
    tokio::fs::create_dir_all(data_dir).await?;
    
    let db_path = data_dir.join("vibe_browser.db");
    let database_url = format!("sqlite:{}", db_path.display());
    
    log::info!("Initializing database at: {}", database_url);
    
    let pool = SqlitePool::connect(&database_url).await?;
    
    // Run migrations
    run_migrations(&pool).await?;
    
    Ok(pool)
}

/// Run database migrations
async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    log::info!("Running database migrations...");
    
    // Create components cache table
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS component_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            component_name TEXT NOT NULL,
            requirements_hash TEXT NOT NULL UNIQUE,
            component_code TEXT NOT NULL,
            description TEXT,
            dependencies TEXT, -- JSON array
            validation_status TEXT, -- JSON object
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_accessed DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#)
    .execute(pool)
    .await?;

    // Create configuration table
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS configuration (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#)
    .execute(pool)
    .await?;

    // Create AI provider configurations table
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS ai_providers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_type TEXT NOT NULL,
            name TEXT NOT NULL UNIQUE,
            api_key TEXT NOT NULL,
            base_url TEXT,
            model TEXT NOT NULL,
            max_tokens INTEGER,
            temperature REAL,
            is_active BOOLEAN DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#)
    .execute(pool)
    .await?;

    // Create generation history table
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS generation_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            requirements TEXT NOT NULL,
            component_name TEXT,
            success BOOLEAN NOT NULL,
            error_message TEXT,
            duration_ms INTEGER,
            provider_used TEXT,
            model_used TEXT,
            tokens_used INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#)
    .execute(pool)
    .await?;

    // Create indices for better performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_component_cache_hash ON component_cache(requirements_hash)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_component_cache_accessed ON component_cache(last_accessed)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_generation_history_session ON generation_history(session_id)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_generation_history_created ON generation_history(created_at)")
        .execute(pool)
        .await?;

    log::info!("Database migrations completed successfully");
    Ok(())
}

/// Get the application data directory
pub fn get_data_dir() -> Result<PathBuf> {
    let data_dir = if cfg!(target_os = "windows") {
        dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .join("VibeBrowser")
    } else if cfg!(target_os = "macos") {
        dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .join("VibeBrowser")
    } else {
        // Linux and other Unix-like systems
        dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .join("vibe-browser")
    };

    Ok(data_dir)
}

/// Database health check
pub async fn health_check(pool: &SqlitePool) -> Result<()> {
    let row = sqlx::query("SELECT 1 as health")
        .fetch_one(pool)
        .await?;
    
    let health: i32 = row.get("health");
    if health != 1 {
        return Err(anyhow::anyhow!("Database health check failed"));
    }
    
    Ok(())
}

/// Clean up old cache entries (older than 30 days)
pub async fn cleanup_old_cache(pool: &SqlitePool) -> Result<u64> {
    let result = sqlx::query(r#"
        DELETE FROM component_cache 
        WHERE last_accessed < datetime('now', '-30 days')
    "#)
    .execute(pool)
    .await?;

    let deleted_count = result.rows_affected();
    if deleted_count > 0 {
        log::info!("Cleaned up {} old cache entries", deleted_count);
    }

    Ok(deleted_count)
}

/// Get database statistics
pub async fn get_database_stats(pool: &SqlitePool) -> Result<DatabaseStats> {
    let component_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM component_cache")
        .fetch_one(pool)
        .await?;

    let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_providers")
        .fetch_one(pool)
        .await?;

    let generation_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generation_history")
        .fetch_one(pool)
        .await?;

    let cache_size_mb = get_database_size(pool).await?;

    Ok(DatabaseStats {
        cached_components: component_count as u64,
        ai_providers: provider_count as u64,
        generation_history_entries: generation_count as u64,
        cache_size_mb,
    })
}

async fn get_database_size(pool: &SqlitePool) -> Result<f64> {
    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(pool)
        .await?;
    
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(pool)
        .await?;
    
    let size_bytes = page_count * page_size;
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
    
    Ok(size_mb)
}

#[derive(Debug, serde::Serialize)]
pub struct DatabaseStats {
    pub cached_components: u64,
    pub ai_providers: u64,
    pub generation_history_entries: u64,
    pub cache_size_mb: f64,
}