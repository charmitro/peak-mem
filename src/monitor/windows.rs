use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;

pub struct WindowsMonitor;

impl WindowsMonitor {
    pub fn new() -> Result<Self> {
        Ok(WindowsMonitor)
    }
}

#[async_trait]
impl MemoryMonitor for WindowsMonitor {
    async fn get_memory_usage(&self, _pid: u32) -> Result<MemoryUsage> {
        // Windows implementation would use GetProcessMemoryInfo
        Err(PeakMemError::UnsupportedPlatform(
            "Windows support not yet implemented".to_string(),
        ))
    }

    async fn get_process_tree(&self, _pid: u32) -> Result<ProcessMemoryInfo> {
        Err(PeakMemError::UnsupportedPlatform(
            "Windows support not yet implemented".to_string(),
        ))
    }

    async fn get_child_pids(&self, _pid: u32) -> Result<Vec<u32>> {
        Err(PeakMemError::UnsupportedPlatform(
            "Windows support not yet implemented".to_string(),
        ))
    }
}
