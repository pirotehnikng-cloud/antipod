use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::CoreError;

pub fn resolve_chrome_exe() -> Result<PathBuf, CoreError> {
    let mut candidates = Vec::new();
    if let Ok(pf) = env::var("PROGRAMFILES") {
        candidates.push(PathBuf::from(pf).join(r"Google\Chrome\Application\chrome.exe"));
    }
    if let Ok(pf86) = env::var("PROGRAMFILES(X86)") {
        candidates.push(PathBuf::from(pf86).join(r"Google\Chrome\Application\chrome.exe"));
    }
    if let Ok(local) = env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join(r"Google\Chrome\Application\chrome.exe"));
    }

    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| CoreError::BinaryNotFound("Google Chrome не найден в системе".to_string()))
}

pub fn open_chrome_profile(
    profile_dir: Option<&str>,
    url: Option<&str>,
) -> Result<u32, CoreError> {
    let chrome = resolve_chrome_exe()?;
    let mut cmd = Command::new(&chrome);

    match profile_dir.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) if p.eq_ignore_ascii_case("incognito") => {
            cmd.arg("--incognito");
        }
        Some(p) => {
            cmd.arg(format!("--profile-directory={p}"));
        }
        None => {
            cmd.arg("--profile-directory=Default");
        }
    }

    cmd.arg(url.unwrap_or("https://accounts.google.com/"));

    let child = cmd
        .spawn()
        .map_err(|e| CoreError::Process(format!("Не удалось запустить Chrome: {e}")))?;
    Ok(child.id())
}
