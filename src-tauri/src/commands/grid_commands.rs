use crate::commands::CommandResponse;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Grid layout configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GridConfig {
    pub id: String,
    pub name: String,
    pub columns: u32,
    pub rows: u32,
    pub gap: u32,
    pub padding: u32,
    pub components: Vec<GridComponent>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Component within a grid layout
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GridComponent {
    pub id: String,
    pub component_name: String,
    pub component_code: String,
    pub position: GridPosition,
    pub props: HashMap<String, serde_json::Value>,
    pub style_overrides: HashMap<String, String>,
}

/// Position of a component within the grid
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GridPosition {
    pub col_start: u32,
    pub col_end: u32,
    pub row_start: u32,
    pub row_end: u32,
}

/// Thread-safe in-memory grid storage (in a real app, this would be in the database)
static GRID_STORAGE: Lazy<Arc<Mutex<HashMap<String, GridConfig>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

fn get_grid_storage() -> Arc<Mutex<HashMap<String, GridConfig>>> {
    GRID_STORAGE.clone()
}

/// Create a new grid configuration
#[tauri::command]
pub async fn create_grid_config(
    name: String,
    columns: u32,
    rows: u32,
    gap: Option<u32>,
    padding: Option<u32>,
) -> Result<CommandResponse<GridConfig>, String> {
    log::info!("Creating new grid config: {} ({}x{})", name, columns, rows);

    let grid_config = GridConfig {
        id: Uuid::new_v4().to_string(),
        name,
        columns,
        rows,
        gap: gap.unwrap_or(16),
        padding: padding.unwrap_or(16),
        components: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    get_grid_storage()
        .lock()
        .unwrap()
        .insert(grid_config.id.clone(), grid_config.clone());

    log::info!("Grid config created with ID: {}", grid_config.id);
    Ok(CommandResponse::success(grid_config))
}

/// Get grid configuration by ID
#[tauri::command]
pub async fn get_grid_config(grid_id: String) -> Result<CommandResponse<GridConfig>, String> {
    let storage = get_grid_storage();
    let storage_guard = storage.lock().unwrap();

    match storage_guard.get(&grid_id) {
        Some(grid_config) => Ok(CommandResponse::success(grid_config.clone())),
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// List all grid configurations
#[tauri::command]
pub async fn list_grid_configs() -> Result<CommandResponse<Vec<GridConfig>>, String> {
    let storage = get_grid_storage();
    let storage_guard = storage.lock().unwrap();
    let configs: Vec<GridConfig> = storage_guard.values().cloned().collect();

    log::info!("Retrieved {} grid configurations", configs.len());
    Ok(CommandResponse::success(configs))
}

/// Update grid configuration
#[tauri::command]
pub async fn update_grid_config(
    grid_id: String,
    name: Option<String>,
    columns: Option<u32>,
    rows: Option<u32>,
    gap: Option<u32>,
    padding: Option<u32>,
) -> Result<CommandResponse<GridConfig>, String> {
    let storage = get_grid_storage();
    let mut storage_guard = storage.lock().unwrap();

    match storage_guard.get_mut(&grid_id) {
        Some(grid_config) => {
            if let Some(name) = name {
                grid_config.name = name;
            }
            if let Some(columns) = columns {
                grid_config.columns = columns;
            }
            if let Some(rows) = rows {
                grid_config.rows = rows;
            }
            if let Some(gap) = gap {
                grid_config.gap = gap;
            }
            if let Some(padding) = padding {
                grid_config.padding = padding;
            }
            grid_config.updated_at = chrono::Utc::now();

            log::info!("Updated grid config: {}", grid_id);
            Ok(CommandResponse::success(grid_config.clone()))
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Delete grid configuration
#[tauri::command]
pub async fn delete_grid_config(grid_id: String) -> Result<CommandResponse<String>, String> {
    let storage = get_grid_storage();
    let mut storage_guard = storage.lock().unwrap();

    match storage_guard.remove(&grid_id) {
        Some(_) => {
            log::info!("Deleted grid config: {}", grid_id);
            Ok(CommandResponse::success(
                "Grid configuration deleted".to_string(),
            ))
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Add component to grid
#[tauri::command]
pub async fn add_component_to_grid(
    grid_id: String,
    component_name: String,
    component_code: String,
    position: GridPosition,
    props: Option<HashMap<String, serde_json::Value>>,
    style_overrides: Option<HashMap<String, String>>,
) -> Result<CommandResponse<GridComponent>, String> {
    log::info!(
        "Adding component '{}' to grid '{}'",
        component_name,
        grid_id
    );

    let storage = get_grid_storage();
    let mut storage_guard = storage.lock().unwrap();

    match storage_guard.get_mut(&grid_id) {
        Some(grid_config) => {
            let component = GridComponent {
                id: Uuid::new_v4().to_string(),
                component_name,
                component_code,
                position,
                props: props.unwrap_or_default(),
                style_overrides: style_overrides.unwrap_or_default(),
            };

            grid_config.components.push(component.clone());
            grid_config.updated_at = chrono::Utc::now();

            log::info!("Component added to grid with ID: {}", component.id);
            Ok(CommandResponse::success(component))
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Update component in grid
#[tauri::command]
pub async fn update_grid_component(
    grid_id: String,
    component_id: String,
    component_name: Option<String>,
    component_code: Option<String>,
    position: Option<GridPosition>,
    props: Option<HashMap<String, serde_json::Value>>,
    style_overrides: Option<HashMap<String, String>>,
) -> Result<CommandResponse<GridComponent>, String> {
    log::info!(
        "Updating component '{}' in grid '{}'",
        component_id,
        grid_id
    );

    let storage = get_grid_storage();
    let mut storage_guard = storage.lock().unwrap();

    match storage_guard.get_mut(&grid_id) {
        Some(grid_config) => {
            match grid_config
                .components
                .iter_mut()
                .find(|c| c.id == component_id)
            {
                Some(component) => {
                    if let Some(name) = component_name {
                        component.component_name = name;
                    }
                    if let Some(code) = component_code {
                        component.component_code = code;
                    }
                    if let Some(pos) = position {
                        component.position = pos;
                    }
                    if let Some(props) = props {
                        component.props = props;
                    }
                    if let Some(styles) = style_overrides {
                        component.style_overrides = styles;
                    }

                    grid_config.updated_at = chrono::Utc::now();

                    log::info!("Component '{}' updated in grid '{}'", component_id, grid_id);
                    Ok(CommandResponse::success(component.clone()))
                }
                None => Ok(CommandResponse::error(
                    "Component not found in grid".to_string(),
                )),
            }
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Remove component from grid
#[tauri::command]
pub async fn remove_component_from_grid(
    grid_id: String,
    component_id: String,
) -> Result<CommandResponse<String>, String> {
    log::info!(
        "Removing component '{}' from grid '{}'",
        component_id,
        grid_id
    );

    let storage = get_grid_storage();
    let mut storage_guard = storage.lock().unwrap();

    match storage_guard.get_mut(&grid_id) {
        Some(grid_config) => {
            let initial_len = grid_config.components.len();
            grid_config.components.retain(|c| c.id != component_id);

            if grid_config.components.len() < initial_len {
                grid_config.updated_at = chrono::Utc::now();
                log::info!(
                    "Component '{}' removed from grid '{}'",
                    component_id,
                    grid_id
                );
                Ok(CommandResponse::success(
                    "Component removed from grid".to_string(),
                ))
            } else {
                Ok(CommandResponse::error(
                    "Component not found in grid".to_string(),
                ))
            }
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Get all components in a grid
#[tauri::command]
pub async fn get_grid_components(
    grid_id: String,
) -> Result<CommandResponse<Vec<GridComponent>>, String> {
    let storage = get_grid_storage();
    let storage_guard = storage.lock().unwrap();

    match storage_guard.get(&grid_id) {
        Some(grid_config) => Ok(CommandResponse::success(grid_config.components.clone())),
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Generate CSS for a grid layout
#[tauri::command]
pub async fn generate_grid_css(grid_id: String) -> Result<CommandResponse<String>, String> {
    let storage = get_grid_storage();
    let storage_guard = storage.lock().unwrap();

    match storage_guard.get(&grid_id) {
        Some(grid_config) => {
            let css = format!(
                r#".grid-container-{} {{
    display: grid;
    grid-template-columns: repeat({}, 1fr);
    grid-template-rows: repeat({}, 1fr);
    gap: {}px;
    padding: {}px;
    width: 100%;
    height: 100%;
}}
"#,
                grid_config.id,
                grid_config.columns,
                grid_config.rows,
                grid_config.gap,
                grid_config.padding
            );

            let mut component_css = String::new();
            for component in &grid_config.components {
                component_css.push_str(&format!(
                    r#"
.grid-item-{} {{
    grid-column: {} / {};
    grid-row: {} / {};
}}
"#,
                    component.id,
                    component.position.col_start,
                    component.position.col_end,
                    component.position.row_start,
                    component.position.row_end
                ));

                // Add style overrides
                if !component.style_overrides.is_empty() {
                    component_css.push_str(&format!("\n.grid-item-{} {{\n", component.id));
                    for (property, value) in &component.style_overrides {
                        component_css.push_str(&format!("    {}: {};\n", property, value));
                    }
                    component_css.push_str("}\n");
                }
            }

            let full_css = format!("{}{}", css, component_css);
            Ok(CommandResponse::success(full_css))
        }
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Export grid configuration as JSON
#[tauri::command]
pub async fn export_grid_config(grid_id: String) -> Result<CommandResponse<String>, String> {
    let storage = get_grid_storage();
    let storage_guard = storage.lock().unwrap();

    match storage_guard.get(&grid_id) {
        Some(grid_config) => match serde_json::to_string_pretty(grid_config) {
            Ok(json) => {
                log::info!("Exported grid config '{}' to JSON", grid_id);
                Ok(CommandResponse::success(json))
            }
            Err(e) => {
                log::error!("Failed to serialize grid config: {}", e);
                Ok(CommandResponse::error(
                    "Failed to export grid configuration".to_string(),
                ))
            }
        },
        None => Ok(CommandResponse::error(
            "Grid configuration not found".to_string(),
        )),
    }
}

/// Import grid configuration from JSON
#[tauri::command]
pub async fn import_grid_config(json_data: String) -> Result<CommandResponse<GridConfig>, String> {
    match serde_json::from_str::<GridConfig>(&json_data) {
        Ok(mut grid_config) => {
            // Generate new ID to avoid conflicts
            grid_config.id = Uuid::new_v4().to_string();
            grid_config.updated_at = chrono::Utc::now();

            get_grid_storage()
                .lock()
                .unwrap()
                .insert(grid_config.id.clone(), grid_config.clone());

            log::info!("Imported grid config with new ID: {}", grid_config.id);
            Ok(CommandResponse::success(grid_config))
        }
        Err(e) => {
            log::error!("Failed to parse grid config JSON: {}", e);
            Ok(CommandResponse::error(
                "Invalid grid configuration JSON".to_string(),
            ))
        }
    }
}
