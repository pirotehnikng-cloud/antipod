use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use antipod_core::{
    config_path, launch_detached, load_config, open_chrome_profile, save_config,
    stop_detached_by_name, AppConfig,
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
    /// Создать стартовый конфиг %APPDATA%\antipod\profiles.toml
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
                "{:<14} {:<5} {:<9} {:<12} {}",
                "ПРОФИЛЬ", "РЕЖИМ", "ЦВЕТ", "CHROME", "ПРОЕКТ"
            );
            for p in cfg.profiles {
                println!(
                    "{:<14} {:<5} {:<9} {:<12} {}",
                    p.name,
                    format!("{:?}", p.mode).to_lowercase(),
                    p.titlebar_color,
                    p.chrome_profile.as_deref().unwrap_or("-"),
                    p.default_project.display()
                );
            }
        }
        Commands::Launch { name, project } => {
            let cfg = load_config()?;
            let inst = launch_detached(&cfg, &name, project)?;
            println!(
                "[RUN] Профиль '{}' запущен (PID: {})",
                inst.profile_name(),
                inst.pid()
            );
        }
        Commands::Chrome { name, url } => {
            let cfg = load_config()?;
            let profile = cfg
                .profiles
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| anyhow::anyhow!("Профиль '{name}' не найден"))?;
            let pid = open_chrome_profile(profile.chrome_profile.as_deref(), url.as_deref())?;
            println!(
                "[CHROME] Открыт профиль '{}' (Chrome: {}, PID: {pid})",
                name,
                profile.chrome_profile.as_deref().unwrap_or("Default")
            );
        }
        Commands::Stop { name } => {
            stop_detached_by_name(&name)?;
            println!("[STOP] Профиль '{}' остановлен", name);
        }
    }

    Ok(())
}
