pub mod browser;
pub mod config;
pub mod discovery;
pub mod isolation;
pub mod job;
pub mod launcher;
pub mod metrics;
pub mod proxy;
pub mod theme;
pub mod worktree;

use thiserror::Error;

pub use browser::{open_chrome_profile, open_chrome_profile_with_proxy, resolve_chrome_exe};
pub use config::{
    app_data_dir, config_path, load_config, remove_profile, save_config, upsert_profile, AppConfig,
    LaunchMode, ProfileConfig,
};
pub use launcher::{
    launch_by_name, launch_by_name_with_proxy, launch_detached, launch_detached_with_proxy,
    stop_detached_by_name, unregister_runtime_state, RunningInstance,
};
pub use metrics::{measure_many, ProcessMetrics};
pub use proxy::{mask_proxy_str, ProxyBridge, ProxyConfig, ProxyScheme};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ошибка JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Ошибка конфигурации: {0}")]
    Config(String),
    #[error("Профиль '{0}' не найден в profiles.toml")]
    ProfileNotFound(String),
    #[error("Бинарник не найден: {0}")]
    BinaryNotFound(String),
    #[error("Ошибка изоляции: {0}")]
    Isolation(String),
    #[error("Ошибка процесса: {0}")]
    Process(String),
    #[error("Ошибка Git: {0}")]
    Git(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_config_roundtrip() {
        let cfg = AppConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: AppConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.profiles.len(), 2);
        assert_eq!(parsed.profiles[0].name, "personal");
    }

    #[test]
    fn windows_drive_splitting() {
        let (drive, rest) = isolation::split_windows_drive(Path::new(r"D:\AntiPod\work\home"));
        assert_eq!(drive, "D:");
        assert_eq!(rest, r"\AntiPod\work\home");
    }

    #[test]
    fn titlebar_color_written() {
        let dir = std::env::temp_dir().join("antipod_test_theme");
        let _ = std::fs::remove_dir_all(&dir);
        theme::apply_titlebar_color(&dir, "#1f4e3d").expect("apply color");

        let raw = std::fs::read_to_string(dir.join("User").join("settings.json")).expect("read");
        assert!(raw.contains("#1f4e3d"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn current_process_measured() {
        let pid = std::process::id();
        let res = measure_many(&[pid]);
        let m = res.get(&pid).expect("current pid metrics");
        assert!(m.alive);
        assert!(m.process_count >= 1);
    }
}
