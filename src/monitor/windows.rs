use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use std::future::Future;
use std::pin::Pin;

pub struct WindowsMonitor;

impl WindowsMonitor {
    pub fn new() -> Result<Self> {
        Ok(WindowsMonitor)
    }
}

impl MemoryMonitor for WindowsMonitor {
    fn get_memory_usage(
        &self,
        _pid: u32,
    ) -> Pin<Box<dyn Future<Output = Result<MemoryUsage>> + Send + '_>> {
        Box::pin(async move {
            // Windows implementation would use GetProcessMemoryInfo
            Err(PeakMemError::UnsupportedPlatform(
                "Windows support not yet implemented".to_string(),
            ))
        })
    }

    fn get_process_tree(
        &self,
        _pid: u32,
    ) -> Pin<Box<dyn Future<Output = Result<ProcessMemoryInfo>> + Send + '_>> {
        Box::pin(async move {
            Err(PeakMemError::UnsupportedPlatform(
                "Windows support not yet implemented".to_string(),
            ))
        })
    }

    fn get_child_pids(
        &self,
        _pid: u32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u32>>> + Send + '_>> {
        Box::pin(async move {
            Err(PeakMemError::UnsupportedPlatform(
                "Windows support not yet implemented".to_string(),
            ))
        })
    }
}
