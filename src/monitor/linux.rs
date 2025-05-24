use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::fs;
use std::path::Path;

pub struct LinuxMonitor;

impl LinuxMonitor {
    pub fn new() -> Result<Self> {
        Ok(LinuxMonitor)
    }

    fn read_proc_status(&self, pid: u32) -> Result<(u64, u64)> {
        let status_path = format!("/proc/{}/status", pid);
        let content = fs::read_to_string(&status_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PeakMemError::ProcessSpawn(format!("Process {} not found", pid))
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                PeakMemError::PermissionDenied(format!("Cannot access process {}", pid))
            } else {
                e.into()
            }
        })?;

        let mut rss_kb = 0u64;
        let mut vsz_kb = 0u64;

        for line in content.lines() {
            if let Some(rss_str) = line.strip_prefix("VmRSS:") {
                rss_kb = parse_kb_value(rss_str)?;
            } else if let Some(vsz_str) = line.strip_prefix("VmSize:") {
                vsz_kb = parse_kb_value(vsz_str)?;
            }
        }

        Ok((rss_kb * 1024, vsz_kb * 1024))
    }

    fn get_process_name(&self, pid: u32) -> String {
        let comm_path = format!("/proc/{}/comm", pid);
        fs::read_to_string(comm_path)
            .unwrap_or_else(|_| format!("pid:{}", pid))
            .trim()
            .to_string()
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
        let children_path = format!("/proc/{}/task/{}/children", pid, pid);

        match fs::read_to_string(&children_path) {
            Ok(content) => {
                let pids: Vec<u32> = content
                    .split_whitespace()
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect();
                Ok(pids)
            }
            Err(_) => {
                // Fallback: scan /proc for processes with matching ppid
                let mut children = Vec::new();
                if let Ok(entries) = fs::read_dir("/proc") {
                    for entry in entries.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            if let Ok(child_pid) = name.parse::<u32>() {
                                if let Ok(stat) = read_proc_stat(child_pid) {
                                    if stat.ppid == pid {
                                        children.push(child_pid);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(children)
            }
        }
    }
}

fn parse_kb_value(s: &str) -> Result<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return Err(PeakMemError::Parse("Empty memory value".to_string()));
    }

    parts[0]
        .parse::<u64>()
        .map_err(|e| PeakMemError::Parse(format!("Failed to parse memory value: {}", e)))
}

struct ProcStat {
    ppid: u32,
}

fn read_proc_stat(pid: u32) -> Result<ProcStat> {
    let stat_path = format!("/proc/{}/stat", pid);
    let content = fs::read_to_string(stat_path)?;

    // Find the last ')' to handle process names with spaces/parentheses
    if let Some(pos) = content.rfind(')') {
        let fields: Vec<&str> = content[pos + 1..].split_whitespace().collect();
        if fields.len() >= 2 {
            let ppid = fields[1]
                .parse::<u32>()
                .map_err(|e| PeakMemError::Parse(format!("Failed to parse ppid: {}", e)))?;
            return Ok(ProcStat { ppid });
        }
    }

    Err(PeakMemError::Parse(
        "Invalid /proc/pid/stat format".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kb_value() {
        assert_eq!(parse_kb_value("1024 kB").unwrap(), 1024);
        assert_eq!(parse_kb_value("  2048   kB  ").unwrap(), 2048);
        assert_eq!(parse_kb_value("0 kB").unwrap(), 0);
    }

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
