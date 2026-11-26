use std::env;
use std::path::PathBuf;

use crate::config::LaunchMode;
use crate::CoreError;

pub fn resolve_antigravity_exe(
    mode: LaunchMode,
    custom_path: Option<&PathBuf>,
) -> Result<PathBuf, CoreError> {
    if let Some(path) = custom_path {
        if path.exists() {
            return Ok(path.clone());
        }
        return Err(CoreError::BinaryNotFound(format!(
            "Указанный файл не найден: {}",
            path.display()
        )));
    }

    let local_app = env::var("LOCALAPPDATA").map_err(|_| {
        CoreError::BinaryNotFound("Переменная среды LOCALAPPDATA не задана".to_string())
    })?;
    let programs = PathBuf::from(local_app).join("Programs");

    let candidates = match mode {
        LaunchMode::Ide => vec![
            programs.join("Antigravity IDE").join("Antigravity IDE.exe"),
            programs.join("Antigravity").join("Antigravity IDE.exe"),
            programs.join("Antigravity").join("Antigravity.exe"),
        ],
        LaunchMode::App => vec![
            programs.join("Antigravity").join("Antigravity.exe"),
            programs.join("Antigravity IDE").join("Antigravity IDE.exe"),
        ],
    };

    candidates.into_iter().find(|p| p.exists()).ok_or_else(|| {
        CoreError::BinaryNotFound(
            "Antigravity не найден в %LOCALAPPDATA%\\Programs. Укажи exe_path в profiles.toml."
                .to_string(),
        )
    })
}
