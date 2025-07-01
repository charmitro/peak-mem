//! Platform-agnostic memory monitoring interface and implementations.
//!
//! This module provides a trait-based abstraction for memory monitoring
//! across different operating systems, along with platform-specific
//! implementations.

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

/// Trait defining the interface for platform-specific memory monitors.
///
/// Each platform must implement this trait to provide memory monitoring
/// capabilities. The trait is async to support potentially blocking
/// system calls without blocking the runtime.
#[async_trait]
pub trait MemoryMonitor: Send + Sync {
    /// Get the current memory usage for a specific process.
    ///
    /// # Arguments
    /// * `pid` - Process ID to monitor
    ///
    /// # Returns
    /// * `Result<MemoryUsage>` - Current memory statistics or error
    async fn get_memory_usage(&self, pid: u32) -> Result<MemoryUsage>;

    /// Get the complete process tree with memory information.
    ///
    /// # Arguments
    /// * `pid` - Root process ID
    ///
    /// # Returns
    /// * `Result<ProcessMemoryInfo>` - Process tree with memory data or error
    async fn get_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo>;

    /// Get the list of child process IDs for a given process.
    ///
    /// # Arguments
    /// * `pid` - Parent process ID
    ///
    /// # Returns
    /// * `Result<Vec<u32>>` - List of child PIDs or error
    #[allow(dead_code)]
    async fn get_child_pids(&self, pid: u32) -> Result<Vec<u32>>;
}

/// Thread-safe shared reference to a memory monitor.
pub type SharedMonitor = Arc<Mutex<Box<dyn MemoryMonitor>>>;

/// Creates a platform-specific memory monitor instance.
///
/// This factory function automatically selects the appropriate monitor
/// implementation based on the compilation target.
///
/// # Returns
/// * `Result<Box<dyn MemoryMonitor>>` - Platform-specific monitor or error
///
/// # Errors
/// * `PeakMemError::UnsupportedPlatform` - Platform not supported
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
