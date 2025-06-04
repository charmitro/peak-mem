use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;
use procfs::process::Process;

pub struct LinuxMonitor;

impl LinuxMonitor {
    pub fn new() -> Result<Self> {
        Ok(LinuxMonitor)
    }

    fn read_proc_status(&self, pid: u32) -> Result<(u64, u64)> {
        let process = Process::new(pid as i32).map_err(|e| match e {
            procfs::ProcError::NotFound(_) => {
                PeakMemError::ProcessSpawn(format!("Process {} not found", pid))
            }
            procfs::ProcError::PermissionDenied(_) => {
                PeakMemError::PermissionDenied(format!("Cannot access process {}", pid))
            }
            _ => PeakMemError::ProcessSpawn(format!("Failed to access process {}: {}", pid, e)),
        })?;

        let status = process.status().map_err(|e| {
            PeakMemError::ProcessSpawn(format!("Failed to read process {} status: {}", pid, e))
        })?;

        let rss_bytes = status.vmrss.unwrap_or(0) * 1024;
        let vsz_bytes = status.vmsize.unwrap_or(0) * 1024;

        Ok((rss_bytes, vsz_bytes))
    }

    fn get_process_name(&self, pid: u32) -> String {
        Process::new(pid as i32)
            .and_then(|p| p.stat())
            .map(|stat| stat.comm)
            .unwrap_or_else(|_| format!("pid:{}", pid))
    }
}

#[async_trait]
impl MemoryMonitor for LinuxMonitor {
    async fn get_memory_usage(&self, pid: u32) -> Result<MemoryUsage> {
        let (rss_bytes, vsz_bytes) = self.read_proc_status(pid)?;

        Ok(MemoryUsage {
            rss_bytes,
            vsz_bytes,
            timestamp: Utc::now(),
        })
    }

    async fn get_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo> {
        let memory = self.get_memory_usage(pid).await?;
        let name = self.get_process_name(pid);
        let child_pids = self.get_child_pids(pid).await?;

        let mut children = Vec::new();
        for child_pid in child_pids {
            if let Ok(child_info) = Box::pin(self.get_process_tree(child_pid)).await {
                children.push(child_info);
            }
        }

        Ok(ProcessMemoryInfo {
            pid,
            name,
            memory,
            children,
        })
    }

    async fn get_child_pids(&self, pid: u32) -> Result<Vec<u32>> {
        let mut children = Vec::new();

        // Use procfs to iterate through all processes
        if let Ok(all_procs) = procfs::process::all_processes() {
            for process in all_procs.flatten() {
                if let Ok(stat) = process.stat() {
                    if stat.ppid == pid as i32 {
                        children.push(stat.pid as u32);
                    }
                }
            }
        }

        Ok(children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_memory_usage_self() {
        let monitor = LinuxMonitor::new().unwrap();
        let pid = std::process::id();

        let usage = monitor.get_memory_usage(pid).await;
        assert!(usage.is_ok());

        let usage = usage.unwrap();
        assert!(usage.rss_bytes > 0);
        assert!(usage.vsz_bytes >= usage.rss_bytes);
    }
}
