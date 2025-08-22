pub mod ai;
pub mod storage;
pub mod commands;

use commands::{ai_commands::AIState, *};
use storage::{initialize_database, get_data_dir, ComponentCache, ConfigManager};
use anyhow::Result;
use log::info;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    env_logger::init();
    info!("Starting Vibe Browser application");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // Initialize async components
            tauri::async_runtime::spawn(async move {
                if let Err(e) = initialize_app_state(app_handle.clone()).await {
                    log::error!("Failed to initialize application state: {}", e);
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // AI Commands
            generate_component,
            stream_component_generation,
            validate_component,
            configure_ai_provider,
            set_active_ai_provider,
            get_ai_providers,
            get_supported_providers,
            delete_ai_provider,
            get_cached_components,
            search_cached_components,
            clear_component_cache,
            get_cache_stats,
            // Grid Commands
            create_grid_config,
            get_grid_config,
            list_grid_configs,
            update_grid_config,
            delete_grid_config,
            add_component_to_grid,
            update_grid_component,
            remove_component_from_grid,
            get_grid_components,
            generate_grid_css,
            export_grid_config,
            import_grid_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn initialize_app_state(app_handle: tauri::AppHandle) -> Result<()> {
    info!("Initializing application state...");
    
    // Get data directory and initialize database
    let data_dir = get_data_dir()?;
    let db_pool = initialize_database(&data_dir).await?;
    
    // Initialize storage components
    let cache = ComponentCache::new(db_pool.clone());
    let config_manager = ConfigManager::new(db_pool.clone());
    
    // Create AI state
    let mut ai_state = AIState::new(cache, config_manager, app_handle.clone());
    
    // Try to initialize orchestrator if there's an active provider
    if let Err(e) = ai_state.initialize_orchestrator().await {
        log::warn!("Could not initialize AI orchestrator on startup: {}", e);
    }
    
    // Store the state in Tauri's state management
    app_handle.manage(ai_state);
    
    info!("Application state initialized successfully");
    Ok(())
}
