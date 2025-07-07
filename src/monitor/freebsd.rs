use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

pub struct FreeBSDMonitor {
    system: std::sync::Mutex<System>,
}

impl FreeBSDMonitor {
    pub fn new() -> Result<Self> {
        Ok(FreeBSDMonitor {
            system: std::sync::Mutex::new(System::new_with_specifics(
                RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
            )),
        })
    }

    fn refresh_process(&self, pid: u32) -> Result<()> {
        let sysinfo_pid = Pid::from_u32(pid);
        let mut system = self.system.lock().unwrap();

        // Use ProcessRefreshKind::everything() to ensure all data including memory is
        // refreshed
        if !system.refresh_process_specifics(sysinfo_pid, ProcessRefreshKind::everything()) {
            return Err(PeakMemError::ProcessSpawn(format!(
                "Process {pid} not found"
            )));
        }

        Ok(())
    }

    fn get_process_info(&self, pid: u32) -> Result<(String, u64, u64)> {
        let sysinfo_pid = Pid::from_u32(pid);
        let system = self.system.lock().unwrap();

        let process = system
            .process(sysinfo_pid)
            .ok_or_else(|| PeakMemError::ProcessSpawn(format!("Process {pid} not found")))?;

        let name = process.name().to_string();
        let rss_bytes = process.memory();
        let vsz_bytes = process.virtual_memory();

        Ok((name, rss_bytes, vsz_bytes))
    }

    async fn build_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo> {
        self.refresh_process(pid)?;
        let (name, rss_bytes, vsz_bytes) = self.get_process_info(pid)?;

        let memory = MemoryUsage {
            rss_bytes,
            vsz_bytes,
            timestamp: Utc::now(),
        };

        // Get child processes
        let sysinfo_pid = Pid::from_u32(pid);
        let child_pids: Vec<u32> = {
            let system = self.system.lock().unwrap();
            system
                .processes()
                .iter()
                .filter_map(|(child_pid, child_process)| {
                    if child_process.parent() == Some(sysinfo_pid) {
                        Some(child_pid.as_u32())
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Build child trees
        let mut children = Vec::new();
        for child_pid in child_pids {
            match Box::pin(self.build_process_tree(child_pid)).await {
                Ok(child_tree) => children.push(child_tree),
                Err(_) => continue, // Child might have exited
            }
        }

        Ok(ProcessMemoryInfo {
            pid,
            name,
            memory,
            children,
        })
    }
}

#[async_trait]
impl MemoryMonitor for FreeBSDMonitor {
    async fn get_memory_usage(&self, pid: u32) -> Result<MemoryUsage> {
        self.refresh_process(pid)?;
        let (_name, rss_bytes, vsz_bytes) = self.get_process_info(pid)?;

        Ok(MemoryUsage {
            rss_bytes,
            vsz_bytes,
            timestamp: Utc::now(),
        })
    }

    async fn get_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo> {
        self.build_process_tree(pid).await
    }

    async fn get_child_pids(&self, pid: u32) -> Result<Vec<u32>> {
        {
            let mut system = self.system.lock().unwrap();
            system.refresh_processes();
        }

        let sysinfo_pid = Pid::from_u32(pid);
        let child_pids: Vec<u32> = {
            let system = self.system.lock().unwrap();
            system
                .processes()
                .iter()
                .filter_map(|(child_pid, child_process)| {
                    if child_process.parent() == Some(sysinfo_pid) {
                        Some(child_pid.as_u32())
                    } else {
                        None
                    }
                })
                .collect()
        };

        Ok(child_pids)
    }
}
