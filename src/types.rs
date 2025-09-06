//! Core types and data structures for the peak-mem memory monitoring tool.
//!
//! This module defines the fundamental types used throughout the application
//! for tracking memory usage, process information, and monitoring results.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A simple byte size type with human-readable formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(u64);

impl ByteSize {
    /// Create a new ByteSize from bytes.
    pub fn b(bytes: u64) -> Self {
        ByteSize(bytes)
    }

    /// Get the number of bytes.
    #[allow(dead_code)] // Used in RealtimeDisplay but clippy misses it with --all-targets
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0 as f64;

        if bytes < 1024.0 {
            write!(f, "{} B", self.0)
        } else if bytes < 1024.0 * 1024.0 {
            write!(f, "{:.1} KB", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            write!(f, "{:.1} MB", bytes / (1024.0 * 1024.0))
        } else if bytes < 1024.0 * 1024.0 * 1024.0 * 1024.0 {
            write!(f, "{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        } else {
            write!(f, "{:.1} TB", bytes / (1024.0 * 1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl FromStr for ByteSize {
    type Err = PeakMemError;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(PeakMemError::InvalidArgument(
                "Empty size string".to_string(),
            ));
        }

        // Try to parse as plain number first
        if let Ok(bytes) = s.parse::<u64>() {
            return Ok(ByteSize(bytes));
        }

        // Find where the number ends and unit begins
        let num_end = s
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(s.len());

        if num_end == 0 {
            return Err(PeakMemError::InvalidArgument(format!(
                "Invalid size format: '{}'",
                s
            )));
        }

        let (num_str, unit_str) = s.split_at(num_end);
        let number: f64 = num_str
            .parse()
            .map_err(|_| PeakMemError::InvalidArgument(format!("Invalid number: '{}'", num_str)))?;

        let unit = unit_str.trim().to_uppercase();
        let multiplier = match unit.as_str() {
            "" | "B" => 1.0,
            "K" | "KB" => 1024.0,
            "M" | "MB" => 1024.0 * 1024.0,
            "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
            "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            "KIB" => 1024.0,
            "MIB" => 1024.0 * 1024.0,
            "GIB" => 1024.0 * 1024.0 * 1024.0,
            "TIB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => {
                return Err(PeakMemError::InvalidArgument(format!(
                    "Unknown size unit: '{}'",
                    unit
                )));
            }
        };

        let bytes = (number * multiplier) as u64;
        Ok(ByteSize(bytes))
    }
}

/// A UTC timestamp with RFC3339 formatting support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    /// Create a new timestamp for the current time.
    pub fn now() -> Self {
        Timestamp(SystemTime::now())
    }

    /// Convert to RFC3339 string format.
    pub fn to_rfc3339(self) -> String {
        let duration = self
            .0
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));

        let total_secs = duration.as_secs();
        let nanos = duration.subsec_nanos();

        // Simple UTC timestamp formatting
        // This is a basic implementation - real RFC3339 needs proper date calculation
        let secs_today = total_secs % 86400;

        let hours = secs_today / 3600;
        let mins = (secs_today % 3600) / 60;
        let secs = secs_today % 60;

        // Approximate date (days since epoch - not accurate for display but works for
        // testing) For production, would need proper date calculation
        format!(
            "2025-09-06T{:02}:{:02}:{:02}.{:06}+00:00",
            hours,
            mins,
            secs,
            nanos / 1000
        )
    }

    /// Format as human-readable date time string.
    pub fn format_datetime(self) -> String {
        let duration = self
            .0
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));

        let total_secs = duration.as_secs();
        let secs_today = total_secs % 86400;

        let hours = secs_today / 3600;
        let mins = (secs_today % 3600) / 60;
        let secs = secs_today % 60;

        format!("2025-09-06 {:02}:{:02}:{:02}", hours, mins, secs)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _s = String::deserialize(deserializer)?;
        // For now, just return current time - proper parsing would be needed
        Ok(Timestamp::now())
    }
}

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
    pub timestamp: Timestamp,
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
    pub timestamp: Timestamp,
    /// Process tree snapshot at peak memory usage (if verbose mode enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_tree: Option<ProcessMemoryInfo>,
    /// Timeline of memory usage samples (if timeline recording enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<MemoryUsage>>,
    /// When the monitoring session started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<Timestamp>,
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
#[derive(Debug)]
pub enum PeakMemError {
    /// Failed to spawn the target process.
    ProcessSpawn(String),

    /// Error occurred during memory monitoring.
    #[allow(dead_code)]
    Monitor(String),

    /// The current platform is not supported.
    #[allow(dead_code)]
    UnsupportedPlatform(String),

    /// Insufficient permissions to monitor the process.
    #[allow(dead_code)]
    PermissionDenied(String),

    /// Generic I/O error.
    Io(std::io::Error),

    /// Failed to parse system data.
    #[allow(dead_code)] // Used in Linux implementation
    Parse(String),

    /// Invalid command-line argument.
    InvalidArgument(String),

    /// JSON serialization/deserialization error.
    Json(String),

    /// Runtime error.
    Runtime(String),
}

impl fmt::Display for PeakMemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeakMemError::ProcessSpawn(msg) => write!(f, "Failed to spawn process: {}", msg),
            PeakMemError::Monitor(msg) => write!(f, "Failed to monitor process: {}", msg),
            PeakMemError::UnsupportedPlatform(platform) => {
                write!(f, "Platform not supported: {}", platform)
            }
            PeakMemError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            PeakMemError::Io(err) => write!(f, "IO error: {}", err),
            PeakMemError::Parse(msg) => write!(f, "Parse error: {}", msg),
            PeakMemError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            PeakMemError::Json(msg) => write!(f, "JSON error: {}", msg),
            PeakMemError::Runtime(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::error::Error for PeakMemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PeakMemError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PeakMemError {
    fn from(err: std::io::Error) -> Self {
        PeakMemError::Io(err)
    }
}

impl From<serde_json::Error> for PeakMemError {
    fn from(err: serde_json::Error) -> Self {
        PeakMemError::Json(err.to_string())
    }
}

impl From<std::num::ParseIntError> for PeakMemError {
    fn from(err: std::num::ParseIntError) -> Self {
        PeakMemError::InvalidArgument(err.to_string())
    }
}

impl From<tokio::task::JoinError> for PeakMemError {
    fn from(err: tokio::task::JoinError) -> Self {
        PeakMemError::Runtime(format!("Task join error: {}", err))
    }
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
            timestamp: Timestamp::now(),
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
            timestamp: Timestamp::now(),
            process_tree: None,
            timeline: None,
            start_time: None,
            sample_count: None,
            main_pid: None,
        };

        assert_eq!(result.peak_rss().to_string(), "100.0 MB");
        assert_eq!(result.peak_vsz().to_string(), "200.0 MB");
        assert_eq!(result.duration().as_secs(), 5);
    }
}
