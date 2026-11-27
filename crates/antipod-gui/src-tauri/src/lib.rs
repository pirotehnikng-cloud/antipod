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
            let proxy_display = antipod_core::mask
