use std::path::PathBuf;
use std::process::Command;

use crate::config::{load_state, save_state, AppConfig, LaunchMode, ProfileConfig};
use crate::discovery::resolve_antigravity_exe;
use crate::isolation::{apply_isolation_env, prepare_profile_dirs};
use crate::job::{kill_process_tree, make_managed_job, JobObject};
use crate::theme::apply_titlebar_color;
use crate::worktree::{git_repo_root, resolve_project_workspace};
use crate::CoreError;

pub struct RunningInstance {
    profile_name: String,
    pid: u32,
    job: Option<JobObject>,
}

impl RunningInstance {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    pub fn terminate(self) -> Result<(), CoreError> {
        let res = match self.job {
            Some(ref job) => job.terminate(),
            None => kill_process_tree(self.pid),
        };
        unregister_runtime_state(&self.profile_name);
        res
    }
}

pub fn launch_by_name(cfg: &AppConfig, name: &str) -> Result<RunningInstance, CoreError> {
    launch_profile_internal(cfg, name, None, true)
}

pub fn launch_detached(
    cfg: &AppConfig,
    name: &str,
    project_override: Option<PathBuf>,
) -> Result<RunningInstance, CoreError> {
    launch_profile_internal(cfg, name, project_override, false)
}

pub fn stop_detached_by_name(name: &str) -> Result<(), CoreError> {
    let mut state = load_state();
    let pid = state
        .pids
        .remove(name)
        .ok_or_else(|| CoreError::Process(format!("Профиль '{name}' не найден среди активных")))?;
    state.active_repos.remove(name);
    let _ = save_state(&state);
    kill_process_tree(pid)
}

pub fn unregister_runtime_state(profile_name: &str) {
    let mut state = load_state();
    state.pids.remove(profile_name);
    state.active_repos.remove(profile_name);
    let _ = save_state(&state);
}

fn launch_profile_internal(
    cfg: &AppConfig,
    name: &str,
    project_override: Option<PathBuf>,
    managed_job: bool,
) -> Result<RunningInstance, CoreError> {
    let profile: &ProfileConfig = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| CoreError::ProfileNotFound(name.to_string()))?;

    let exe = resolve_antigravity_exe(profile.mode, cfg.exe_path.as_ref())?;
    let requested_project = project_override.unwrap_or_else(|| profile.default_project.clone());

    let mut state = load_state();
    let effective_project = resolve_project_workspace(
        &cfg.profiles_root,
        &profile.name,
        &requested_project,
        cfg.auto_git_worktree,
        &state,
    )?;

    let paths = prepare_profile_dirs(
        &cfg.profiles_root,
        &profile.name,
        cfg.isolate_extensions,
        &effective_project,
    )?;

    apply_titlebar_color(&paths.user_data, &profile.titlebar_color)?;

    let mut cmd = Command::new(&exe);
    cmd.arg(format!("--user-data-dir={}", paths.user_data.display()));

    if profile.mode == LaunchMode::Ide {
        cmd.arg(format!("--extensions-dir={}", paths.extensions.display()))
            .arg(&effective_project);
    }

    cmd.current_dir(&effective_project);
    apply_isolation_env(&mut cmd, &paths.home, cfg.share_dev_toolchains);

    let child = cmd
        .spawn()
        .map_err(|e| CoreError::Process(format!("Ошибка запуска {}: {e}", exe.display())))?;
    let pid = child.id();

    let job = if managed_job {
        Some(make_managed_job(pid)?)
    } else {
        None
    };

    state.pids.insert(profile.name.clone(), pid);
    if let Some(root) = git_repo_root(&requested_project) {
        state.active_repos.insert(profile.name.clone(), root);
    }
    let _ = save_state(&state);

    Ok(RunningInstance {
        profile_name: profile.name.clone(),
        pid,
        job,
    })
}
