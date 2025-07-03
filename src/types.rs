//! Core types and data structures for the peak-mem memory monitoring tool.
//!
//! This module defines the fundamental types used throughout the application
//! for tracking memory usage, process information, and monitoring results.

use bytesize::ByteSize;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Represents a snapshot of memory usage at a specific point in time.
///
/// This struct captures both RSS (Resident Set Size) and VSZ (Virtual Size)
/// memory metrics along with a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Physical memory currently used by the process (in bytes).
    pub rss_bytes: u64,
    /// Virtual memory size of the process (in bytes).
    pub vsz_bytes: u64,
    /// When this measurement was taken.
    pub timestamp: DateTime<Utc>,
}

/// Hierarchical representation of a process and its children's memory usage.
///
/// This struct forms a tree structure where each node contains information
/// about a process and its direct children, enabling visualization of memory
/// usage across an entire process tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMemoryInfo {
    /// Process ID of this process.
    pub pid: u32,
    /// Name or command of the process.
    pub name: String,
    /// Current memory usage of this process.
    pub memory: MemoryUsage,
    /// List of child processes and their memory information.
    pub children: Vec<ProcessMemoryInfo>,
}

/// Complete results from monitoring a process's memory usage.
///
/// This struct contains all the data collected during a monitoring session,
/// including peak memory usage, duration, optional timeline data, and process
/// tree information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorResult {
    /// The command that was executed.
    pub command: String,
    /// Peak RSS (Resident Set Size) observed during execution (in bytes).
    pub peak_rss_bytes: u64,
    /// Peak VSZ (Virtual Size) observed during execution (in bytes).
    pub peak_vsz_bytes: u64,
    /// Total duration of the monitoring session (in milliseconds).
    pub duration_ms: u64,
    /// Exit code of the monitored process, if it completed.
    pub exit_code: Option<i32>,
    /// Whether the memory usage exceeded the configured threshold.
    pub threshold_exceeded: bool,
    /// When the monitoring session completed.
    pub timestamp: DateTime<Utc>,
    /// Process tree snapshot at peak memory usage (if verbose mode enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_tree: Option<ProcessMemoryInfo>,
    /// Timeline of memory usage samples (if timeline recording enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<MemoryUsage>>,
    /// When the monitoring session started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    /// Number of memory samples collected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<u64>,
    /// Process ID of the main monitored process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_pid: Option<u32>,
}

impl MonitorResult {
    /// Returns the peak RSS as a human-readable ByteSize.
    pub fn peak_rss(&self) -> ByteSize {
        ByteSize::b(self.peak_rss_bytes)
    }

    /// Returns the peak VSZ as a human-readable ByteSize.
    pub fn peak_vsz(&self) -> ByteSize {
        ByteSize::b(self.peak_vsz_bytes)
    }

    /// Returns the monitoring duration as a Duration type.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }
}

/// Error types that can occur during memory monitoring operations.
///
/// This enum provides structured error handling for all failure modes
/// in the peak-mem application.
#[derive(Error, Debug)]
pub enum PeakMemError {
    /// Failed to spawn the target process.
    #[error("Failed to spawn process: {0}")]
    ProcessSpawn(String),

    /// Error occurred during memory monitoring.
    #[error("Failed to monitor process: {0}")]
    #[allow(dead_code)]
    Monitor(String),

    /// The current platform is not supported.
    #[error("Platform not supported: {0}")]
    #[allow(dead_code)]
    UnsupportedPlatform(String),

    /// Insufficient permissions to monitor the process.
    #[error("Permission denied: {0}")]
    #[allow(dead_code)]
    PermissionDenied(String),

    /// Generic I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse system data.
    #[error("Parse error: {0}")]
    #[allow(dead_code)] // Used in Linux implementation
    Parse(String),
}

/// Type alias for Results that may contain PeakMemError.
pub type Result<T> = std::result::Result<T, PeakMemError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_usage_creation() {
        let usage = MemoryUsage {
            rss_bytes: 1024 * 1024,
            vsz_bytes: 2048 * 1024,
            timestamp: Utc::now(),
        };

        assert_eq!(usage.rss_bytes, 1024 * 1024);
        assert_eq!(usage.vsz_bytes, 2048 * 1024);
    }

    #[test]
    fn test_monitor_result_conversions() {
        let result = MonitorResult {
            command: "test".to_string(),
            peak_rss_bytes: 100 * 1024 * 1024,
            peak_vsz_bytes: 200 * 1024 * 1024,
            duration_ms: 5000,
            exit_code: Some(0),
            threshold_exceeded: false,
            timestamp: Utc::now(),
            process_tree: None,
            timeline: None,
            start_time: None,
            sample_count: None,
            main_pid: None,
        };

        assert_eq!(result.peak_rss().to_string(), "104.9 MB");
        assert_eq!(result.peak_vsz().to_string(), "209.7 MB");
        assert_eq!(result.duration().as_secs(), 5);
    }
}
