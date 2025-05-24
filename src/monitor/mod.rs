use crate::types::{MemoryUsage, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod tracker;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "freebsd")]
pub mod freebsd;

#[async_trait]
pub trait MemoryMonitor: Send + Sync {
    async fn get_memory_usage(&self, pid: u32) -> Result<MemoryUsage>;

    async fn get_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo>;

    async fn get_child_pids(&self, pid: u32) -> Result<Vec<u32>>;
}

pub type SharedMonitor = Arc<Mutex<Box<dyn MemoryMonitor>>>;

pub fn create_monitor() -> Result<Box<dyn MemoryMonitor>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxMonitor::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSMonitor::new()?))
    }

    #[cfg(windows)]
    {
        Ok(Box::new(windows::WindowsMonitor::new()?))
    }

    #[cfg(target_os = "freebsd")]
    {
        Ok(Box::new(freebsd::FreeBSDMonitor::new()?))
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        windows,
        target_os = "freebsd"
    )))]
    {
        Err(crate::types::PeakMemError::UnsupportedPlatform(
            std::env::consts::OS.to_string(),
        ))
    }
}
