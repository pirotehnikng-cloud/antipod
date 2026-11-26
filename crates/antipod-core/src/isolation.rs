use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::CoreError;

pub struct IsolatedPaths {
    pub home: PathBuf,
    pub user_data: PathBuf,
    pub extensions: PathBuf,
}

pub fn prepare_profile_dirs(
    profiles_root: &Path,
    profile_name: &str,
    isolate_extensions: bool,
    project_dir: &Path,
) -> Result<IsolatedPaths, CoreError> {
    let profile_root = profiles_root.join(profile_name);
    let home = profile_root.join("home");
    let user_data = profile_root.join("user-data");
    let extensions = if isolate_extensions {
        profile_root.join("extensions")
    } else {
        profiles_root.join("_shared-extensions")
    };

    fs::create_dir_all(&home)?;
    fs::create_dir_all(&user_data)?;
    fs::create_dir_all(&extensions)?;
    fs::create_dir_all(project_dir)?;

    ensure_junction(&home.join("Projects"), project_dir)?;

    if let Ok(real_home) = env::var("USERPROFILE") {
        let ssh_target = PathBuf::from(real_home).join(".ssh");
        ensure_junction(&home.join(".ssh"), &ssh_target)?;
    }

    Ok(IsolatedPaths {
        home,
        user_data,
        extensions,
    })
}

pub fn apply_isolation_env(cmd: &mut Command, profile_home: &Path, share_dev_toolchains: bool) {
    let (drive, home_path) = split_windows_drive(profile_home);

    cmd.env("USERPROFILE", profile_home)
        .env("HOME", profile_home)
        .env("HOMEDRIVE", drive)
        .env("HOMEPATH", home_path)
        .env("SSH_CONNECTION", "127.0.0.1 1234 127.0.0.1 22");

    if !share_dev_toolchains {
        return;
    }

    if let Ok(real_home_str) = env::var("USERPROFILE") {
        let real_home = PathBuf::from(real_home_str);

        let git_cfg = real_home.join(".gitconfig");
        if git_cfg.exists() && env::var_os("GIT_CONFIG_GLOBAL").is_none() {
            cmd.env("GIT_CONFIG_GLOBAL", git_cfg);
        }
        let cargo_home = real_home.join(".cargo");
        if cargo_home.exists() && env::var_os("CARGO_HOME").is_none() {
            cmd.env("CARGO_HOME", cargo_home);
        }
        let rustup_home = real_home.join(".rustup");
        if rustup_home.exists() && env::var_os("RUSTUP_HOME").is_none() {
            cmd.env("RUSTUP_HOME", rustup_home);
        }
    }
}

pub fn split_windows_drive(path: &Path) -> (String, String) {
    let s = path.to_string_lossy();
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        (s[..2].to_string(), s[2..].to_string())
    } else {
        ("C:".to_string(), s.to_string())
    }
}

#[cfg(windows)]
fn ensure_junction(link: &Path, target: &Path) -> Result<(), CoreError> {
    if target.exists() && !link.exists() {
        junction::create(target, link).map_err(|e| {
            CoreError::Isolation(format!(
                "Не удалось создать NTFS junction {}: {e}",
                link.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn ensure_junction(_link: &Path, _target: &Path) -> Result<(), CoreError> {
    Ok(())
}
