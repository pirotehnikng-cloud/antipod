use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::RuntimeState;
use crate::CoreError;

pub fn resolve_project_workspace(
    profiles_root: &Path,
    profile_name: &str,
    requested_project: &Path,
    auto_worktree: bool,
    state: &RuntimeState,
) -> Result<PathBuf, CoreError> {
    if !auto_worktree {
        return Ok(requested_project.to_path_buf());
    }

    let Some(repo_root) = git_repo_root(requested_project) else {
        return Ok(requested_project.to_path_buf());
    };

    let occupied_by_other = state
        .active_repos
        .iter()
        .any(|(other, path)| other != profile_name && path == &repo_root);

    if !occupied_by_other {
        return Ok(requested_project.to_path_buf());
    }

    create_or_reuse_worktree(profiles_root, profile_name, &repo_root)
}

pub fn git_repo_root(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

fn create_or_reuse_worktree(
    profiles_root: &Path,
    profile_name: &str,
    repo_root: &Path,
) -> Result<PathBuf, CoreError> {
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");

    let wt_parent = profiles_root.join(profile_name).join("worktrees");
    fs::create_dir_all(&wt_parent)?;
    let wt_path = wt_parent.join(repo_name);

    if wt_path.join(".git").exists() {
        return Ok(wt_path);
    }

    let branch = format!("antipod/{profile_name}");
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "add", "-B", &branch])
        .arg(&wt_path)
        .status()
        .map_err(|e| CoreError::Git(format!("Не удалось выполнить git worktree: {e}")))?;

    if !status.success() {
        return Err(CoreError::Git(format!(
            "git worktree add завершился с ошибкой для ветки {branch}"
        )));
    }

    Ok(wt_path)
}
