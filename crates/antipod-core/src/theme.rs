use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::CoreError;

pub fn apply_titlebar_color(user_data_dir: &Path, hex_color: &str) -> Result<(), CoreError> {
    if hex_color.trim().is_empty() {
        return Ok(());
    }

    let settings_dir = user_data_dir.join("User");
    fs::create_dir_all(&settings_dir)?;
    let settings_path = settings_dir.join("settings.json");

    let mut config: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if let Some(obj) = config.as_object_mut() {
        let colors = obj
            .entry("workbench.colorCustomizations")
            .or_insert_with(|| json!({}));
        if let Some(map) = colors.as_object_mut() {
            map.insert("titleBar.activeBackground".to_string(), json!(hex_color));
            map.insert("titleBar.inactiveBackground".to_string(), json!(hex_color));
        }
    }

    let formatted = serde_json::to_string_pretty(&config)?;
    fs::write(settings_path, formatted)?;
    Ok(())
}
