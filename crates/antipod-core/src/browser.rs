use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::CoreError;

fn exe_name_lower(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

fn is_chromium_exe(path: &Path) -> bool {
    matches!(
        exe_name_lower(path).as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "brave.exe"
            | "chromium.exe"
            | "vivaldi.exe"
            | "opera.exe"
            | "browser.exe"
    )
}

fn is_firefox_exe(path: &Path) -> bool {
    matches!(
        exe_name_lower(path).as_str(),
        "firefox.exe" | "librewolf.exe" | "waterfox.exe"
    )
}

pub fn resolve_chrome_exe() -> Result<PathBuf, CoreError> {
    resolve_browser_exe(None)
}

pub fn resolve_browser_exe(custom_path: Option<&Path>) -> Result<PathBuf, CoreError> {
    if let Some(custom) = custom_path.filter(|p| !p.as_os_str().is_empty()) {
        if custom.exists() {
            return Ok(custom.to_path_buf());
        }
        return Err(CoreError::BinaryNotFound(format!(
            "Указанный браузер не найден: {}",
            custom.display()
        )));
    }

    let mut candidates = Vec::new();
    let pf = env::var("PROGRAMFILES").ok();
    let pf86 = env::var("PROGRAMFILES(X86)").ok();
    let local = env::var("LOCALAPPDATA").ok();

    let relative_paths = [
        r"Google\Chrome\Application\chrome.exe",
        r"Microsoft\Edge\Application\msedge.exe",
        r"BraveSoftware\Brave-Browser\Application\brave.exe",
        r"Mozilla Firefox\firefox.exe",
    ];

    for rel in relative_paths {
        if let Some(ref base) = pf {
            candidates.push(PathBuf::from(base).join(rel));
        }
        if let Some(ref base) = pf86 {
            candidates.push(PathBuf::from(base).join(rel));
        }
        if let Some(ref base) = local {
            candidates.push(PathBuf::from(base).join(rel));
        }
    }

    candidates.into_iter().find(|p| p.exists()).ok_or_else(|| {
        CoreError::BinaryNotFound(
            "Браузер не найден (Chrome, Edge, Brave или Firefox). Укажи путь к .exe браузера в настройках профиля."
                .to_string(),
        )
    })
}

pub fn open_chrome_profile(
    profile_dir: Option<&str>,
    url: Option<&str>,
) -> Result<u32, CoreError> {
    open_browser_for_profile(None, profile_dir, url, None, None)
}

pub fn open_chrome_profile_with_proxy(
    profile_dir: Option<&str>,
    url: Option<&str>,
    proxy_url: Option<&str>,
    isolated_data_dir: Option<&Path>,
) -> Result<u32, CoreError> {
    open_browser_for_profile(None, profile_dir, url, proxy_url, isolated_data_dir)
}

pub fn open_browser_for_profile(
    browser_path: Option<&Path>,
    profile_dir: Option<&str>,
    url: Option<&str>,
    proxy_url: Option<&str>,
    isolated_data_dir: Option<&Path>,
) -> Result<u32, CoreError> {
    let browser = resolve_browser_exe(browser_path)?;
    let mut cmd = Command::new(&browser);

    if is_chromium_exe(&browser) {
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
    } else if is_firefox_exe(&browser) {
        cmd.arg(url.unwrap_or("https://accounts.google.com/"));
    } else if let Some(target_url) = url {
        // Антидетект-браузеры и сторонние лаунчеры запускаются чисто без Chromium-флагов
        cmd.arg(target_url);
    }

    let child = cmd.spawn().map_err(|e| {
        CoreError::Process(format!("Не удалось запустить {}: {e}", browser.display()))
    })?;
    Ok(child.id())
}
