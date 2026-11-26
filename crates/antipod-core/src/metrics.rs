use std::collections::{HashMap, HashSet};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, Copy)]
pub struct ProcessMetrics {
    pub alive: bool,
    pub ram_mb: u64,
    pub process_count: usize,
}

pub fn measure_many(root_pids: &[u32]) -> HashMap<u32, ProcessMetrics> {
    let mut result = HashMap::with_capacity(root_pids.len());
    if root_pids.is_empty() {
        return result;
    }

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );

    let processes = sys.processes();

    for &root_u32 in root_pids {
        let root = Pid::from_u32(root_u32);
        if !processes.contains_key(&root) {
            result.insert(
                root_u32,
                ProcessMetrics {
                    alive: false,
                    ram_mb: 0,
                    process_count: 0,
                },
            );
            continue;
        }

        let mut tree: HashSet<Pid> = HashSet::new();
        tree.insert(root);

        loop {
            let mut added = false;
            for (pid, proc_info) in processes {
                if !tree.contains(pid) {
                    if let Some(parent) = proc_info.parent() {
                        if tree.contains(&parent) {
                            tree.insert(*pid);
                            added = true;
                        }
                    }
                }
            }
            if !added {
                break;
            }
        }

        let total_bytes: u64 = tree
            .iter()
            .filter_map(|pid| processes.get(pid))
            .map(|p| p.memory())
            .sum();

        result.insert(
            root_u32,
            ProcessMetrics {
                alive: true,
                ram_mb: total_bytes / (1024 * 1024),
                process_count: tree.len(),
            },
        );
    }

    result
}
