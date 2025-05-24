use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;

pub struct FreeBSDMonitor;

impl FreeBSDMonitor {
    pub fn new() -> Result<Self> {
        Ok(FreeBSDMonitor)
    }
}

#[async_trait]
impl MemoryMonitor for FreeBSDMonitor {
    async fn get_memory_usage(&self, _pid: u32) -> Result<MemoryUsage> {
        // FreeBSD implementation would use sysctl
        Err(PeakMemError::UnsupportedPlatform(
            "FreeBSD support not yet implemented".to_string(),
        ))
    }

    async fn get_process_tree(&self, _pid: u32) -> Result<ProcessMemoryInfo> {
        Err(PeakMemError::UnsupportedPlatform(
            "FreeBSD support not yet implemented".to_string(),
        ))
    }

    async fn get_child_pids(&self, _pid: u32) -> Result<Vec<u32>> {
        Err(PeakMemError::UnsupportedPlatform(
            "FreeBSD support not yet implemented".to_string(),
        ))
    }
}
