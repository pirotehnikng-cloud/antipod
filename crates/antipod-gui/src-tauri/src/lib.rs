use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use antipod_core::{LaunchMode, ProfileConfig};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent, Wry,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const TRAY_ID: &str = "antipod-tray";
const PROFILE_PREFIX: &str = "profile:";
const RELEASES_URL: &str = "https://github.com/pirotehnikng-cloud/antipod/releases";

struct ActiveBridge {
    raw_proxy: String,
    bridge: antipod_core::ProxyBridge,
}

#[derive(Default)]
struct AppState {
    running: Mutex<HashMap<String, antipod_core::RunningInstance>>,
    bridges: Mutex<HashMap<String, ActiveBridge>>,
}

#[derive(Serialize)]
struct ProfileStatus {
    name: String,
    mode: LaunchMode,
    pid: Option<u32>,
    ram_mb: Option<u64>,
    process_count: Option<usize>,
    titlebar_color: String,
    default_project: String,
    chrome_profile: Option<String>,
    browser_path: Option<String>,
    mostlogin_profile_id: Option<String>,
    proxy: Option<String>,
    proxy_display: Option<String>,
}

#[derive(Serialize)]
struct MostLoginSettings {
    api_url: String,
    api_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProfileInput {
    name: String,
    mode: LaunchMode,
    titlebar_color: String,
    default_project: String,
    chrome_profile: Option<String>,
    browser_path: Option<String>,
    mostlogin_profile_id: Option<String>,
    proxy: Option<String>,
}

fn open_url_in_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Не удалось открыть браузер: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Открытие браузера поддерживается только на Windows".to_string())
    }
}

fn ensure_proxy_url(
    state: &AppState,
    profile_name: &str,
    raw_proxy: Option<&str>,
) -> Result<Option<String>, String> {
    let mut bridges = state.bridges.lock().map_err(|e| e.to_string())?;
    let Some(trimmed) = raw_proxy.map(str::trim).filter(|s| !s.is_empty()) else {
        bridges.remove(profile_name);
        return Ok(None);
    };

    if let Some(local_url) = bridges
        .get(profile_name)
        .filter(|b| b.raw_proxy == trimmed)
        .map(|b| b.bridge.local_url())
    {
        return Ok(Some(local_url));
    }

    let parsed = antipod_core::ProxyConfig::parse(trimmed).map_err(|e| e.to_string())?;
    let bridge = antipod_core::ProxyBridge::start(parsed).map_err(|e| e.to_string())?;
    let local_url = bridge.local_url();
    bridges.insert(
        profile_name.to_string(),
        ActiveBridge {
            raw_proxy: trimmed.to_string(),
            bridge,
        },
    );
    Ok(Some(local_url))
}

fn collect_live_metrics(
    running: &mut HashMap<String, antipod_core::RunningInstance>,
) -> HashMap<String, (u32, u64, usize)> {
    let pids: Vec<u32> = running.values().map(|inst| inst.pid()).collect();
    let metrics = antipod_core::measure_many(&pids);

    let mut dead_names = Vec::new();
    let mut live = HashMap::new();

    for (name, inst) in running.iter() {
        let pid = inst.pid();
        match metrics.get(&pid) {
            Some(m) if m.alive => {
                live.insert(name.clone(), (pid, m.ram_mb, m.process_count));
            }
            _ => dead_names.push(name.clone()),
        }
    }

    for name in dead_names {
        running.remove(&name);
        antipod_core::unregister_runtime_state(&name);
    }

    live
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let profiles_submenu = Submenu::new(app, "Профили", true)?;
    let profiles = antipod_core::load_config()
        .map(|c| c.profiles)
        .unwrap_or_default();

    let live_map = app
        .try_state::<AppState>()
        .and_then(|s| s.running.lock().ok().map(|mut m| collect_live_metrics(&mut m)))
        .unwrap_or_default();

    if profiles.is_empty() {
        let empty =
            MenuItem::with_id(app, "no_profiles", "Профили не настроены", false, None::<&str>)?;
        profiles_submenu.append(&empty)?;
    } else {
        for p in profiles {
            let label = match live_map.get(&p.name) {
                Some((_, ram_mb, _)) => format!("● {} ({ram_mb} MB) — остановить", p.name),
                None => format!("○ {} — запустить", p.name),
            };
            let item = MenuItem::with_id(
                app,
                format!("{PROFILE_PREFIX}{}", p.name),
                label,
                true,
                None::<&str>,
            )?;
            profiles_submenu.append(&item)?;
        }
    }

    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "Запускать с Windows",
        true,
        autostart_enabled,
        None::<&str>,
    )?;

    let show_item = MenuItem::with_id(app, "show", "Открыть AntiPod", true, None::<&str>)?;
    let releases_item =
        MenuItem::with_id(app, "open_releases", "Релизы на GitHub", true, None::<&str>)?;
    let stop_all_item =
        MenuItem::with_id(app, "stop_all", "Остановить все профили", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[
            &show_item,
            &profiles_submenu,
            &sep1,
            &autostart_item,
            &releases_item,
            &stop_all_item,
            &sep2,
            &quit_item,
        ],
    )
}

fn refresh_tray(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn toggle_profile_from_tray(app: &AppHandle, name: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let already_running = state
        .running
        .lock()
        .map(|m| m.contains_key(name))
        .unwrap_or(false);

    if already_running {
        if let Ok(mut map) = state.running.lock() {
            if let Some(inst) = map.remove(name) {
                let _ = inst.terminate();
            }
        }
        if let Ok(mut bridges) = state.bridges.lock() {
            bridges.remove(name);
        }
    } else if let Ok(cfg) = antipod_core::load_config() {
        let raw_proxy = cfg
            .profiles
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.proxy.as_deref());
        let Ok(local_proxy) = ensure_proxy_url(&state, name, raw_proxy) else {
            return;
        };
        if let Ok(inst) =
            antipod_core::launch_by_name_with_proxy(&cfg, name, local_proxy.as_deref())
        {
            if let Ok(mut map) = state.running.lock() {
                map.insert(name.to_string(), inst);
            }
        }
    }

    refresh_tray(app);
}

fn stop_all_instances(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut map) = state.running.lock() {
            for (_, instance) in map.drain() {
                let _ = instance.terminate();
            }
        }
        if let Ok(mut bridges) = state.bridges.lock() {
            bridges.clear();
        }
    }
    refresh_tray(app);
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileStatus>, String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    let mut running = state.running.lock().map_err(|e| e.to_string())?;
    let live = collect_live_metrics(&mut running);

    Ok(cfg
        .profiles
        .into_iter()
        .map(|p| {
            let stats = live.get(&p.name).copied();
            let proxy_display = antipod_core::mask_proxy_str(p.proxy.as_deref());
            ProfileStatus {
                name: p.name,
                mode: p.mode,
                pid: stats.map(|s| s.0),
                ram_mb: stats.map(|s| s.1),
                process_count: stats.map(|s| s.2),
                titlebar_color: p.titlebar_color,
                default_project: p.default_project.to_string_lossy().to_string(),
                chrome_profile: p.chrome_profile,
                browser_path: p.browser_path.map(|bp| bp.to_string_lossy().to_string()),
                mostlogin_profile_id: p.mostlogin_profile_id,
                proxy: p.proxy,
                proxy_display,
            }
        })
        .collect())
}

#[tauri::command]
fn launch_profile(name: String, app: AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Профиль '{name}' не найден"))?;
    let local_proxy = ensure_proxy_url(&state, &profile.name, profile.proxy.as_deref())?;
    let instance = antipod_core::launch_by_name_with_proxy(&cfg, &name, local_proxy.as_deref())
        .map_err(|e| e.to_string())?;
    let pid = instance.pid();

    state
        .running
        .lock()
        .map_err(|e| e.to_string())?
        .insert(name, instance);

    refresh_tray(&app);
    Ok(pid)
}

#[tauri::command]
fn stop_profile(name: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(instance) = state.running.lock().map_err(|e| e.to_string())?.remove(&name) {
        instance.terminate().map_err(|e| e.to_string())?;
    }
    if let Ok(mut bridges) = state.bridges.lock() {
        bridges.remove(&name);
    }
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn open_chrome_oauth(name: String, state: State<'_, AppState>) -> Result<u32, String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| format!("Профиль '{name}' не найден"))?;

    if let Some(ml_id) = profile
        .mostlogin_profile_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
    {
        let api_url = cfg.mostlogin_api_url;
        let api_token = cfg.mostlogin_api_token;
        let info = tauri::async_runtime::spawn_blocking(move || {
            antipod_core::open_mostlogin_profile(
                Some(&api_url),
                api_token.as_deref(),
                &ml_id,
                None,
            )
        })
        .await
        .map_err(|e| format!("Ошибка потока MostLogin API: {e}"))?
        .map_err(|e| e.to_string())?;

        if info.port.is_some() {
            antipod_core::set_mostlogin_cdp_port(&profile.name, info.port);
        }
        return Ok(info.pid.unwrap_or(0));
    }

    let local_proxy = ensure_proxy_url(&state, &profile.name, profile.proxy.as_deref())?;
    let chrome_data = cfg.profiles_root.join(&profile.name).join("chrome-data");
    let isolated_dir = local_proxy.as_ref().map(|_| chrome_data.as_path());

    antipod_core::open_browser_for_profile(
        profile.browser_path.as_deref(),
        profile.chrome_profile.as_deref(),
        None,
        local_proxy.as_deref(),
        isolated_dir,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_mostlogin_window(name: String) -> Result<(), String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    let ml_id = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.mostlogin_profile_id.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("У профиля '{name}' не указан MostLogin profileId"))?;

    let api_url = cfg.mostlogin_api_url;
    let api_token = cfg.mostlogin_api_token;
    tauri::async_runtime::spawn_blocking(move || {
        antipod_core::close_mostlogin_profile(Some(&api_url), api_token.as_deref(), &ml_id)
    })
    .await
    .map_err(|e| format!("Ошибка потока MostLogin API: {e}"))?
    .map_err(|e| e.to_string())?;

    antipod_core::set_mostlogin_cdp_port(&name, None);
    Ok(())
}

#[tauri::command]
fn get_mostlogin_settings() -> Result<MostLoginSettings, String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    Ok(MostLoginSettings {
        api_url: cfg.mostlogin_api_url,
        api_token: cfg.mostlogin_api_token,
    })
}

#[tauri::command]
fn save_mostlogin_config(api_url: String, api_token: Option<String>) -> Result<(), String> {
    antipod_core::save_mostlogin_settings(api_url, api_token).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn fetch_mostlogin_profiles(
    api_url: String,
    api_token: Option<String>,
) -> Result<Vec<antipod_core::MostLoginProfileInfo>, String> {
    let url = api_url.trim().to_string();
    let token = api_token
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    tauri::async_runtime::spawn_blocking(move || {
        antipod_core::list_mostlogin_profiles(
            (!url.is_empty()).then_some(url.as_str()),
            token.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Ошибка потока MostLogin API: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_intercept_oauth() -> Result<bool, String> {
    let cfg = antipod_core::load_config().map_err(|e| e.to_string())?;
    Ok(cfg.intercept_oauth)
}

#[tauri::command]
fn set_intercept_oauth(enabled: bool) -> Result<(), String> {
    antipod_core::save_intercept_oauth(enabled).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn poll_oauth_notice() -> Option<String> {
    antipod_core::take_last_oauth_notice()
}

#[tauri::command]
async fn test_proxy_connection(proxy: String) -> Result<antipod_core::ProxyTestResult, String> {
    let trimmed = proxy.trim().to_string();
    if trimmed.is_empty() {
        return Err("Введи адрес прокси для проверки".to_string());
    }
    let parsed = antipod_core::ProxyConfig::parse(&trimmed).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || antipod_core::test_proxy(&parsed))
        .await
        .map_err(|e| format!("Ошибка потока проверки прокси: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_profile(
    input: SaveProfileInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let trimmed = input.name.trim();
    if trimmed.is_empty() {
        return Err("Имя профиля не может быть пустым".to_string());
    }

    let normalized_proxy = input
        .proxy
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref raw) = normalized_proxy {
        antipod_core::ProxyConfig::parse(raw).map_err(|e| e.to_string())?;
    }

    let normalized_browser = input
        .browser_path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let normalized_ml_id = input
        .mostlogin_profile_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    antipod_core::upsert_profile(ProfileConfig {
        name: trimmed.to_string(),
        mode: input.mode,
        titlebar_color: input.titlebar_color,
        default_project: PathBuf::from(input.default_project),
        chrome_profile: input.chrome_profile,
        browser_path: normalized_browser,
        mostlogin_profile_id: normalized_ml_id,
        proxy: normalized_proxy.clone(),
    })
    .map_err(|e| e.to_string())?;

    if let Ok(mut bridges) = state.bridges.lock() {
        let changed = bridges
            .get(trimmed)
            .is_some_and(|b| normalized_proxy.as_deref() != Some(b.raw_proxy.as_str()));
        if changed {
            bridges.remove(trimmed);
        }
    }

    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn delete_profile(name: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(inst) = state.running.lock().map_err(|e| e.to_string())?.remove(&name) {
        let _ = inst.terminate();
    }
    if let Ok(mut bridges) = state.bridges.lock() {
        bridges.remove(&name);
    }
    antipod_core::set_mostlogin_cdp_port(&name, None);
    antipod_core::remove_profile(&name).map_err(|e| e.to_string())?;
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(enabled: bool, app: AppHandle) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn open_releases() -> Result<(), String> {
    open_url_in_browser(RELEASES_URL)
}

pub fn run() {
    if let Some(url) = antipod_core::extract_url_from_args(std::env::args()) {
        let _ = antipod_core::handle_intercepted_url(&url);
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(AppState::default())
        .setup(|app| {
            let start_minimized = std::env::args().any(|arg| arg == "--minimized");
            if !start_minimized {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                }
            }

            let menu = build_tray_menu(app.handle())?;

            let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("AntiPod — Antigravity Launcher")
                .menu(&menu)
                .show_menu_on_left_click(false);

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            tray_builder
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    if let Some(profile_name) = id.strip_prefix(PROFILE_PREFIX) {
                        toggle_profile_from_tray(app, profile_name);
                        return;
                    }
                    match id {
                        "show" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                        "autostart" => {
                            let manager = app.autolaunch();
                            let currently = manager.is_enabled().unwrap_or(false);
                            let _ = if currently {
                                manager.disable()
                            } else {
                                manager.enable()
                            };
                            refresh_tray(app);
                        }
                        "open_releases" => {
                            let _ = open_url_in_browser(RELEASES_URL);
                        }
                        "stop_all" => stop_all_instances(app),
                        "quit" => {
                            stop_all_instances(app);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            launch_profile,
            stop_profile,
            open_chrome_oauth,
            close_mostlogin_window,
            get_mostlogin_settings,
            save_mostlogin_config,
            fetch_mostlogin_profiles,
            get_intercept_oauth,
            set_intercept_oauth,
            poll_oauth_notice,
            test_proxy_connection,
            save_profile,
            delete_profile,
            get_autostart,
            set_autostart,
            open_releases
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка запуска AntiPod GUI");
}
