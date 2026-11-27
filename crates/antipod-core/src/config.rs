use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::CoreError;

const PORTABLE_FLAG_FILE: &str = "portable.flag";
const CONFIG_FILE: &str = "profiles.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    #[default]
    Ide,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    #[serde(default)]
    pub mode: LaunchMode,
    #[serde(default = "default_color")]
    pub titlebar_color: String,
    pub default_project: PathBuf,
    #[serde(default)]
    pub chrome_profile: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_profiles_root")]
    pub profiles_root: PathBuf,
    #[serde(default)]
    pub exe_path: Option<PathBuf>,
    #[serde(default)]
    pub isolate_extensions: bool,
    #[serde(default = "default_true")]
    pub share_dev_toolchains: bool,
    #[serde(default = "default_true")]
    pub auto_git_worktree: bool,
    #[serde(default)]
    pub profiles: Vec<ProfileConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            profiles_root: default_profiles_root(),
            exe_path: None,
            isolate_extensions: false,
            share_dev_toolchains: true,
            auto_git_worktree: true,
            profiles: vec![
                ProfileConfig {
                    name: "personal".to_string(),
                    mode: LaunchMode::Ide,
                    titlebar_color: "#1f4e3d".to_string(),
                    default_project: PathBuf::from(r"C:\Projects"),
                    chrome_profile: Some("Profile 1".to_string()),
                    proxy: None,
                },
                ProfileConfig {
                    name: "work".to_string(),
                    mode: LaunchMode::Ide,
                    titlebar_color: "#2b3a55".to_string(),
                    default_project: PathBuf::from(r"C:\Projects"),
                    chrome_profile: Some("Profile 2".to_string()),
                    proxy: None,
                },
            ],
        }
    }
}

fn default_color() -> String {
    "#2b3a55".to_string()
}

fn default_profiles_root() -> PathBuf {
    if let Some(dir) = portable_dir() {
        dir.join("AntiPodProfiles")
    } else {
        PathBuf::from(r"C:\AntiPodProfiles")
    }
}

fn default_true() -> bool {
    true
}

fn is_portable_dir(dir: &Path) -> bool {
    dir.join(PORTABLE_FLAG_FILE).exists() || dir.join(CONFIG_FILE).exists()
}

pub fn portable_dir() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    if is_portable_dir(dir) {
        Some(dir.to_path_buf())
    } else {
        None
    }
}

pub fn app_data_dir() -> Result<PathBuf, CoreError> {
    if let Some(dir) = portable_dir() {
        return Ok(dir);
    }
    let dirs = BaseDirs::new()
        .ok_or_else(|| CoreError::Config("Не удалось определить каталог APPDATA".to_string()))?;
    Ok(dirs.config_dir().join("antipod"))
}

pub fn config_path() -> Result<PathBuf, CoreError> {
    Ok(app_data_dir()?.join(CONFIG_FILE))
}

pub fn state_path() -> Result<PathBuf, CoreError> {
    Ok(app_data_dir()?.join("state.json"))
}

pub fn load_config() -> Result<AppConfig, CoreError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = fs::read_to_string(&path)?;
    toml::from_str(&raw).map_err(|e| CoreError::Config(format!("Ошибка разбора TOML: {e}")))
}

pub fn save_config(cfg: &AppConfig) -> Result<PathBuf, CoreError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = toml::to_string_pretty(cfg)
        .map_err(|e| CoreError::Config(format!("Ошибка сериализации TOML: {e}")))?;
    fs::write(&path, serialized)?;
    Ok(path)
}

pub fn upsert_profile(profile: ProfileConfig) -> Result<AppConfig, CoreError> {
    let mut cfg = load_config()?;
    if let Some(existing) = cfg.profiles.iter_mut().find(|p| p.name == profile.name) {
        *existing = profile;
    } else {
        cfg.profiles.push(profile);
    }
    save_config(&cfg)?;
    Ok(cfg)
}

pub fn remove_profile(name: &str) -> Result<AppConfig, CoreError> {
    let mut cfg = load_config()?;
    cfg.profiles.retain(|p| p.name != name);
    save_config(&cfg)?;
    Ok(cfg)
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RuntimeState {
    pub pids: HashMap<String, u32>,
    pub active_repos: HashMap<String, PathBuf>,
}

pub fn load_state() -> RuntimeState {
    let Ok(path) = state_path() else {
        return RuntimeState::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return RuntimeState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_state(state: &RuntimeState) -> Result<(), CoreError> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(state)?;
    fs::write(path, serialized)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_portable_flag_and_config() {
        let base = env::temp_dir().join("antipod_portable_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create temp dir");
        assert!(!is_portable_dir(&base));

        let flag = base.join(PORTABLE_FLAG_FILE);
        fs::write(&flag, "").expect("write flag");
        assert!(is_portable_dir(&base));

        fs::remove_file(&flag).expect("remove flag");
        fs::write(base.join(CONFIG_FILE), "").expect("write config");
        assert!(is_portable_dir(&base));

        let _ = fs::remove_dir_all(&base);
    }
}
