use bytesize::ByteSize;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub rss_bytes: u64,
    pub vsz_bytes: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMemoryInfo {
    pub pid: u32,
    pub name: String,
    pub memory: MemoryUsage,
    pub children: Vec<ProcessMemoryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorResult {
    pub command: String,
    pub peak_rss_bytes: u64,
    pub peak_vsz_bytes: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub threshold_exceeded: bool,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_tree: Option<ProcessMemoryInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<MemoryUsage>>,
}

impl MonitorResult {
    pub fn peak_rss(&self) -> ByteSize {
        ByteSize::b(self.peak_rss_bytes)
    }

    pub fn peak_vsz(&self) -> ByteSize {
        ByteSize::b(self.peak_vsz_bytes)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }
}

#[derive(Error, Debug)]
pub enum PeakMemError {
    #[error("Failed to spawn process: {0}")]
    ProcessSpawn(String),

    #[error("Failed to monitor process: {0}")]
    #[allow(dead_code)]
    Monitor(String),

    #[error("Platform not supported: {0}")]
    #[allow(dead_code)]
    UnsupportedPlatform(String),

    #[error("Permission denied: {0}")]
    #[allow(dead_code)]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    #[allow(dead_code)] // Used in Linux implementation
    Parse(String),
}

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
        };

        assert_eq!(result.peak_rss().to_string(), "100.0 MiB");
        assert_eq!(result.peak_vsz().to_string(), "200.0 MiB");
        assert_eq!(result.duration().as_secs(), 5);
    }
}
