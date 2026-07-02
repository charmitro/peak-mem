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
            write!(f, "{:.1} KiB", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            write!(f, "{:.1} MiB", bytes / (1024.0 * 1024.0))
        } else if bytes < 1024.0 * 1024.0 * 1024.0 * 1024.0 {
            write!(f, "{:.1} GiB", bytes / (1024.0 * 1024.0 * 1024.0))
        } else {
            write!(f, "{:.1} TiB", bytes / (1024.0 * 1024.0 * 1024.0 * 1024.0))
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

        // Units without an 'i' are decimal (SI, powers of 1000); units
        // with an 'i' are binary (IEC, powers of 1024).
        let unit = unit_str.trim().to_uppercase();
        let multiplier = match unit.as_str() {
            "" | "B" => 1.0,
            "K" | "KB" => 1000.0,
            "M" | "MB" => 1000.0 * 1000.0,
            "G" | "GB" => 1000.0 * 1000.0 * 1000.0,
            "T" | "TB" => 1000.0 * 1000.0 * 1000.0 * 1000.0,
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

        let (year, month, day) = civil_from_days((total_secs / 86400) as i64);
        let secs_today = total_secs % 86400;

        let hours = secs_today / 3600;
        let mins = (secs_today % 3600) / 60;
        let secs = secs_today % 60;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            year,
            month,
            day,
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
        let (year, month, day) = civil_from_days((total_secs / 86400) as i64);
        let secs_today = total_secs % 86400;

        let hours = secs_today / 3600;
        let mins = (secs_today % 3600) / 60;
        let secs = secs_today % 60;

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month, day, hours, mins, secs
        )
    }

    /// Parses an RFC3339 timestamp string, as written by
    /// [`Timestamp::to_rfc3339`].
    ///
    /// Accepts an optional fractional-seconds part and either `Z` or a
    /// numeric UTC offset, which is normalized away.
    fn parse_rfc3339(s: &str) -> Result<Self> {
        let invalid = || PeakMemError::Parse(format!("invalid RFC3339 timestamp: '{s}'"));

        let b = s.as_bytes();
        if b.len() < 20
            || b[4] != b'-'
            || b[7] != b'-'
            || !matches!(b[10], b'T' | b't' | b' ')
            || b[13] != b':'
            || b[16] != b':'
        {
            return Err(invalid());
        }

        let digits = |range: std::ops::Range<usize>| -> Option<u64> {
            let part = s.get(range)?;
            if !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()) {
                part.parse().ok()
            } else {
                None
            }
        };

        let year = digits(0..4).ok_or_else(invalid)? as i64;
        let month = digits(5..7).ok_or_else(invalid)? as u32;
        let day = digits(8..10).ok_or_else(invalid)? as u32;
        let hour = digits(11..13).ok_or_else(invalid)?;
        let min = digits(14..16).ok_or_else(invalid)?;
        let sec = digits(17..19).ok_or_else(invalid)?;

        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || min > 59
            || sec > 60
        {
            return Err(invalid());
        }

        // Optional fractional seconds, truncated to nanosecond precision.
        let mut idx = 19;
        let mut nanos = 0u32;
        if b[idx] == b'.' {
            let start = idx + 1;
            idx = start;
            while idx < b.len() && b[idx].is_ascii_digit() {
                idx += 1;
            }
            let frac = &s[start..idx.min(start + 9)];
            if frac.is_empty() {
                return Err(invalid());
            }
            nanos = frac.parse().map_err(|_| invalid())?;
            for _ in frac.len()..9 {
                nanos *= 10;
            }
        }

        // Timezone: 'Z' or a +HH:MM/-HH:MM offset.
        let offset_secs = match b.get(idx).copied() {
            Some(b'Z') | Some(b'z') if idx + 1 == b.len() => 0i64,
            Some(sign @ (b'+' | b'-')) if idx + 6 == b.len() && b[idx + 3] == b':' => {
                let oh = digits(idx + 1..idx + 3).ok_or_else(invalid)?;
                let om = digits(idx + 4..idx + 6).ok_or_else(invalid)?;
                if oh > 23 || om > 59 {
                    return Err(invalid());
                }
                let secs = (oh * 3600 + om * 60) as i64;
                if sign == b'-' {
                    -secs
                } else {
                    secs
                }
            }
            _ => return Err(invalid()),
        };

        let days = days_from_civil(year, month, day);
        let utc_secs = days * 86400 + (hour * 3600 + min * 60 + sec) as i64 - offset_secs;

        let time = if utc_secs >= 0 {
            UNIX_EPOCH + Duration::new(utc_secs as u64, nanos)
        } else {
            UNIX_EPOCH - Duration::from_secs(utc_secs.unsigned_abs()) + Duration::new(0, nanos)
        };

        Ok(Timestamp(time))
    }
}

/// Converts days since the Unix epoch to a (year, month, day) civil date.
///
/// Based on Howard Hinnant's `civil_from_days` algorithm
/// (https://howardhinnant.github.io/date_algorithms.html), valid for
/// all dates in the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // year of era [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // March-based month [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Converts a (year, month, day) civil date to days since the Unix epoch.
///
/// Inverse of [`civil_from_days`], from the same source.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // year of era [0, 399]
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64; // March-based month [0, 11]
    let doy = (153 * mp + 2) / 5 + day as i64 - 1; // day of year [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era [0, 146096]
    era * 146_097 + doe - 719_468
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
        let s = String::deserialize(deserializer)?;
        Timestamp::parse_rfc3339(&s).map_err(serde::de::Error::custom)
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
    fn test_byte_size_parsing() {
        // Plain numbers are bytes
        assert_eq!("512".parse::<ByteSize>().unwrap(), ByteSize::b(512));

        // Units without 'i' are decimal (SI)
        assert_eq!("1KB".parse::<ByteSize>().unwrap(), ByteSize::b(1000));
        assert_eq!("1K".parse::<ByteSize>().unwrap(), ByteSize::b(1000));
        assert_eq!(
            "512M".parse::<ByteSize>().unwrap(),
            ByteSize::b(512_000_000)
        );
        assert_eq!(
            "1G".parse::<ByteSize>().unwrap(),
            ByteSize::b(1_000_000_000)
        );
        assert_eq!(
            "1.5GB".parse::<ByteSize>().unwrap(),
            ByteSize::b(1_500_000_000)
        );

        // Units with 'i' are binary (IEC)
        assert_eq!("1KiB".parse::<ByteSize>().unwrap(), ByteSize::b(1024));
        assert_eq!("1MiB".parse::<ByteSize>().unwrap(), ByteSize::b(1_048_576));
        assert_eq!(
            "1GiB".parse::<ByteSize>().unwrap(),
            ByteSize::b(1_073_741_824)
        );

        assert!("1XB".parse::<ByteSize>().is_err());
        assert!("".parse::<ByteSize>().is_err());
    }

    #[test]
    fn test_byte_size_display() {
        assert_eq!(ByteSize::b(512).to_string(), "512 B");
        assert_eq!(ByteSize::b(1024).to_string(), "1.0 KiB");
        assert_eq!(ByteSize::b(1_048_576).to_string(), "1.0 MiB");
        assert_eq!(ByteSize::b(1_073_741_824).to_string(), "1.0 GiB");
    }

    #[test]
    fn test_civil_from_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(11016), (2000, 2, 29)); // leap day
        assert_eq!(civil_from_days(11017), (2000, 3, 1));
        assert_eq!(civil_from_days(19875), (2024, 6, 1));
    }

    #[test]
    fn test_timestamp_formatting() {
        // 2024-06-01T12:34:56.5Z
        let ts = Timestamp(UNIX_EPOCH + Duration::new(1_717_245_296, 500_000_000));
        assert_eq!(ts.to_rfc3339(), "2024-06-01T12:34:56.500000+00:00");
        assert_eq!(ts.format_datetime(), "2024-06-01 12:34:56");

        let epoch = Timestamp(UNIX_EPOCH);
        assert_eq!(epoch.to_rfc3339(), "1970-01-01T00:00:00.000000+00:00");
    }

    #[test]
    fn test_days_from_civil() {
        for days in [-1, 0, 11016, 11017, 19875, -141428] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn test_timestamp_parse_rfc3339() {
        let ts = Timestamp(UNIX_EPOCH + Duration::new(1_717_245_296, 500_000_000));

        // Round-trip through the serialized format
        assert_eq!(Timestamp::parse_rfc3339(&ts.to_rfc3339()).unwrap(), ts);

        // 'Z' suffix and second-only precision
        let z = Timestamp::parse_rfc3339("2024-06-01T12:34:56Z").unwrap();
        assert_eq!(
            z,
            Timestamp(UNIX_EPOCH + Duration::from_secs(1_717_245_296))
        );

        // Numeric offsets are normalized to UTC
        let east = Timestamp::parse_rfc3339("2024-06-01T14:34:56+02:00").unwrap();
        assert_eq!(east, z);
        let west = Timestamp::parse_rfc3339("2024-06-01T05:04:56-07:30").unwrap();
        assert_eq!(west, z);

        assert!(Timestamp::parse_rfc3339("not a timestamp").is_err());
        assert!(Timestamp::parse_rfc3339("2024-13-01T00:00:00Z").is_err());
        assert!(Timestamp::parse_rfc3339("2024-06-01T24:00:00Z").is_err());
        assert!(Timestamp::parse_rfc3339("2024-06-01T12:34:56").is_err()); // missing tz
        assert!(Timestamp::parse_rfc3339("2024-06-01T12:34:56.Z").is_err()); // empty fraction
    }

    #[test]
    fn test_timestamp_serde_round_trip() {
        let ts = Timestamp(UNIX_EPOCH + Duration::new(1_717_245_296, 123_456_000));
        let json = serde_json::to_string(&ts).unwrap();
        let back: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ts);
    }

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

        assert_eq!(result.peak_rss().to_string(), "100.0 MiB");
        assert_eq!(result.peak_vsz().to_string(), "200.0 MiB");
        assert_eq!(result.duration().as_secs(), 5);
    }
}
