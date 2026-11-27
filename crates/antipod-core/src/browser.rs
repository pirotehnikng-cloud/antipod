use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    open_chrome_profile_with_proxy(profile_dir, url, None, None)
}

pub fn open_chrome_profile_with_proxy(
    profile_dir: Option<&str>,
    url: Option<&str>,
    proxy_url: Option<&str>,
    isolated_data_dir: Option<&Path>,
) -> Result<u32, CoreError> {
    let chrome = resolve_chrome_exe()?;
    let mut cmd = Command::new(&chrome);

    let is_incognito = profile_dir
        .map(str::trim)
        .is_some_and(|p| p.eq_ignore_ascii_case("incognito"));

    if let Some(proxy) = proxy_url.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(data_dir) = isolated_data_dir {
            fs::create_dir_all(data_dir)?;
            cmd.arg(format!("--user-data-dir={}", data_dir.display()))
                .arg("--no-first-run")
                .arg("--no-default-browser-check");
        }
        cmd.arg(format!("--proxy-server={proxy}"))
            .arg("--proxy-bypass-list=localhost;127.0.0.1;[::1]")
            .arg("--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1");

        if is_incognito {
            cmd.arg("--incognito");
        }
    } else {
        match profile_dir.map(str::trim).filter(|s| !s.is_empty()) {
            Some(_) if is_incognito => {
                cmd.arg("--incognito");
            }
            Some(p) => {
                cmd.arg(format!("--profile-directory={p}"));
            }
            None => {
                cmd.arg("--profile-directory=Default");
            }
        }
    }

    cmd.arg(url.unwrap_or("https://accounts.google.com/"));

    let child = cmd
        .spawn()
        .map_err(|e| CoreError::Process(format!("Не удалось запустить Chrome: {e}")))?;
    Ok(child.id())
}
