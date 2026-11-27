use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use antipod_core::{
    config_path, launch_detached_with_proxy, load_config, mask_proxy_str, measure_many,
    open_chrome_profile_with_proxy, save_config, stop_detached_by_name, AppConfig, ProxyBridge,
    ProxyConfig,
};

#[derive(Parser)]
#[command(
    name = "antipod",
    version,
    about = "Isolated multi-account launcher for Google Antigravity on Windows"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Создать стартовый конфиг profiles.toml
    Init {
        #[arg(long)]
        force: bool,
    },
    /// Показать список настроенных профилей
    List,
    /// Запустить изолированный профиль Antigravity
    Launch {
        name: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Открыть привязанный профиль Google Chrome для OAuth
    Chrome {
        name: String,
        #[arg(short, long)]
        url: Option<String>,
    },
    /// Остановить дерево процессов запущенного профиля
    Stop { name: String },
}

fn start_proxy_bridge(raw_proxy: Option<&str>) -> Result<Option<ProxyBridge>> {
    let Some(raw) = raw_proxy.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let proxy_cfg = ProxyConfig::parse(raw)?;
    let bridge = ProxyBridge::start(proxy_cfg.clone())?;
    println!(
        "[PROXY] Локальный мост {} -> {}",
        bridge.local_url(),
        proxy_cfg.masked_display()
    );
    Ok(Some(bridge))
}

fn wait_until_exit(pid: u32) {
    loop {
        thread::sleep(Duration::from_secs(2));
        let stats = measure_many(&[pid]);
        if !stats.get(&pid).is_some_and(|m| m.alive) {
            break;
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force } => {
            let path = config_path()?;
            if path.exists() && !force {
                println!(
                    "Конфиг уже существует: {}\nИспользуй --force для перезаписи.",
                    path.display()
                );
                return Ok(());
            }
            let saved = save_config(&AppConfig::default())?;
            println!("[OK] Конфиг создан: {}", saved.display());
        }
        Commands::List => {
            let cfg = load_config()?;
            if cfg.profiles.is_empty() {
                println!("Профили не настроены. Выполни: antipod init");
                return Ok(());
            }
            println!(
                "{:<14} {:<5} {:<9} {:<12} {:<28} {}",
                "ПРОФИЛЬ", "РЕЖИМ", "ЦВЕТ", "CHROME", "ПРОКСИ", "ПРОЕКТ"
            );
            for p in cfg.profiles {
                let proxy_col =
                    mask_proxy_str(p.proxy.as_deref()).unwrap_or_else(|| "-".to_string());
                println!(
                    "{:<14} {:<5} {:<9} {:<12} {:<28} {}",
                    p.name,
                    format!("{:?}", p.mode).to_lowercase(),
                    p.titlebar_color,
                    p.chrome_profile.as_deref().unwrap_or("-"),
                    proxy_col,
                    p.default_project.display()
                );
            }
        }
        Commands::Launch { name, project } => {
            let cfg = load_config()?;
            let profile = cfg
                .profiles
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| anyhow::anyhow!("Профиль '{name}' не найден"))?;

            let bridge = start_proxy_bridge(profile.proxy.as_deref())?;
            let local_proxy = bridge.as_ref().map(ProxyBridge::local_url);

            let inst = launch_detached_with_proxy(&cfg, &name, project, local_proxy.as_deref())?;
            let pid = inst.pid();
            println!(
                "[RUN] Профиль '{}' запущен (PID: {pid})",
                inst.profile_name()
            );

            if let Some(_keep_alive) = bridge {
                println!(
                    "Прокси-мост активен. Не закрывай это окно терминала, пока работает профиль."
                );
                wait_until_exit(pid);
            }
        }
        Commands::Chrome { name, url } => {
            let cfg = load_config()?;
            let profile = cfg
                .profiles
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| anyhow::anyhow!("Профиль '{name}' не найден"))?;

            let bridge = start_proxy_bridge(profile.proxy.as_deref())?;
            let local_proxy = bridge.as_ref().map(ProxyBridge::local_url);
            let chrome_data = cfg.profiles_root.join(&profile.name).join("chrome-data");
            let isolated_dir = local_proxy.as_ref().map(|_| chrome_data.as_path());

            let pid = open_chrome_profile_with_proxy(
                profile.chrome_profile.as_deref(),
                url.as_deref(),
                local_proxy.as_deref(),
                isolated_dir,
            )?;
            println!(
                "[CHROME] Открыт профиль '{}' (Chrome: {}, PID: {pid})",
                name,
                profile.chrome_profile.as_deref().unwrap_or("Default")
            );

            if let Some(_keep_alive) = bridge {
                println!(
                    "Прокси-мост активен. Закрой окно Chrome или нажми Ctrl+C для завершения."
                );
                wait_until_exit(pid);
            }
        }
        Commands::Stop { name } => {
            stop_detached_by_name(&name)?;
            println!("[STOP] Профиль '{}' остановлен", name);
        }
    }

    Ok(())
}
