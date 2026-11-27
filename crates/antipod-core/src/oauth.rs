use std::env;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::browser::open_browser_for_profile;
use crate::config::{app_data_dir, load_config, load_state, set_mostlogin_cdp_port};
use crate::mostlogin::{open_mostlogin_profile, open_url_via_cdp};
use crate::CoreError;

const LAST_OAUTH_FILE: &str = "last_oauth.txt";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn extract_url_from_args<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        let s = arg.as_ref().trim();
        if s == "--open-url" {
            if let Some(next) = iter.next() {
                let candidate = next.as_ref().trim();
                if is_http_url(candidate) {
                    return Some(candidate.to_string());
                }
            }
            continue;
        }
        if is_http_url(s) {
            return Some(s.to_string());
        }
    }
    None
}

fn is_http_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn apply_external_browser_setting(
    user_data_dir: &Path,
    browser_exe: Option<&Path>,
) -> Result<(), CoreError> {
    let user_dir = user_data_dir.join("User");
    fs::create_dir_all(&user_dir)?;
    let settings_path = user_dir.join("settings.json");

    let mut root: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&raw).unwrap_or_else(|_| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    if !root.is_object() {
        root = Value::Object(Map::new());
    }

    if let Some(obj) = root.as_object_mut() {
        if let Some(exe) = browser_exe {
            let cleaned = exe
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string();
            obj.insert(
                "workbench.externalBrowser".to_string(),
                Value::String(cleaned),
            );
        } else {
            obj.remove("workbench.externalBrowser");
        }
    }

    let formatted = serde_json::to_string_pretty(&root)?;
    fs::write(settings_path, formatted)?;
    Ok(())
}

#[cfg(windows)]
pub fn copy_to_clipboard(text: &str) -> Result<(), CoreError> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut child = Command::new("clip")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| CoreError::Process(format!("Не удалось запустить clip.exe: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(CoreError::Process(
            "clip.exe завершился с ошибкой при копировании URL".to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn copy_to_clipboard(_text: &str) -> Result<(), CoreError> {
    Ok(())
}

pub fn handle_intercepted_url(url: &str) -> Result<String, CoreError> {
    let copied = copy_to_clipboard(url).is_ok();
    let clip_prefix = if copied {
        "скопирована в буфер обмена"
    } else {
        "перехвачена"
    };

    let state = load_state();
    let profile_name = env::var("ANTIPOD_PROFILE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| state.last_launched_profile.clone());

    let cfg = load_config()?;
    let profile = profile_name
        .as_deref()
        .and_then(|name| cfg.profiles.iter().find(|p| p.name == name));

    let notice = match profile {
        Some(p) => {
            if let Some(ml_id) = p
                .mostlogin_profile_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let opened_via_cdp = state
                    .mostlogin_cdp_ports
                    .get(&p.name)
                    .copied()
                    .is_some_and(|port| open_url_via_cdp(port, url).is_ok());

                if opened_via_cdp {
                    format!(
                        "OAuth-ссылка для \"{}\" {clip_prefix} и открыта в активном окне MostLogin",
                        p.name
                    )
                } else {
                    match open_mostlogin_profile(
                        Some(&cfg.mostlogin_api_url),
                        cfg.mostlogin_api_token.as_deref(),
                        ml_id,
                        Some(url),
                    ) {
                        Ok(info) => {
                            if info.port.is_some() {
                                set_mostlogin_cdp_port(&p.name, info.port);
                            }
                            format!(
                                "OAuth-ссылка для \"{}\" {clip_prefix} и открыта в MostLogin",
                                p.name
                            )
                        }
                        Err(e) => format!(
                            "OAuth-ссылка для \"{}\" {clip_prefix}, но MostLogin API не ответил ({e}) — вставь вручную через Ctrl+V",
                            p.name
                        ),
                    }
                }
            } else {
                let current_exe = env::current_exe().ok();
                let safe_browser = p
                    .browser_path
                    .as_deref()
                    .filter(|bp| current_exe.as_deref().map_or(true, |cur| cur != *bp));

                match open_browser_for_profile(
                    safe_browser,
                    p.chrome_profile.as_deref(),
                    Some(url),
                    None,
                    None,
                ) {
                    Ok(_) => format!(
                        "OAuth-ссылка для \"{}\" {clip_prefix} и открыта в выбранном браузере",
                        p.name
                    ),
                    Err(e) => format!(
                        "OAuth-ссылка для \"{}\" {clip_prefix}, но браузер не открылся ({e}) — вставь через Ctrl+V",
                        p.name
                    ),
                }
            }
        }
        None => format!("OAuth-ссылка {clip_prefix} (профиль не определён) — вставь в MostLogin через Ctrl+V"),
    };

    save_last_oauth_notice(&notice);
    Ok(notice)
}

fn save_last_oauth_notice(msg: &str) {
    if let Ok(dir) = app_data_dir() {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join(LAST_OAUTH_FILE), msg);
    }
}

pub fn take_last_oauth_notice() -> Option<String> {
    let path = app_data_dir().ok()?.join(LAST_OAUTH_FILE);
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    let trimmed = content.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_direct_and_flagged_urls() {
        assert_eq!(
            extract_url_from_args(["AntiPod.exe", "https://accounts.google.com/o/oauth2/auth"]),
            Some("https://accounts.google.com/o/oauth2/auth".to_string())
        );
        assert_eq!(
            extract_url_from_args(["AntiPod.exe", "--open-url", "http://127.0.0.1:8080/cb"]),
            Some("http://127.0.0.1:8080/cb".to_string())
        );
        assert_eq!(
            extract_url_from_args(["AntiPod.exe", "--minimized"]),
            None
        );
    }

    #[test]
    fn writes_and_clears_external_browser_setting() {
        let dir = env::temp_dir().join("antipod_external_browser_test");
        let _ = fs::remove_dir_all(&dir);
        let exe = Path::new(r"C:\Tools\AntiPod\AntiPod.exe");

        apply_external_browser_setting(&dir, Some(exe)).expect("set browser");
        let raw = fs::read_to_string(dir.join("User").join("settings.json")).expect("read");
        assert!(raw.contains("workbench.externalBrowser"));
        assert!(raw.contains("AntiPod.exe"));

        apply_external_browser_setting(&dir, None).expect("clear browser");
        let raw2 = fs::read_to_string(dir.join("User").join("settings.json")).expect("read2");
        assert!(!raw2.contains("workbench.externalBrowser"));

        let _ = fs::remove_dir_all(&dir);
    }
}
