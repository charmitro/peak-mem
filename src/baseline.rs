//! Baseline comparison functionality for detecting memory usage regressions.
//!
//! This module provides functionality to save memory usage snapshots as
//! baselines and compare new measurements against them to detect regressions.

use crate::types::{MonitorResult, PeakMemError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Represents a saved baseline measurement for comparison.
///
/// Baselines capture key metrics from a monitoring session along with
/// metadata about the environment where the measurement was taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Version of peak-mem that created this baseline.
    pub version: String,
    /// When this baseline was created.
    pub created_at: DateTime<Utc>,
    /// Command that was monitored.
    pub command: String,
    /// Peak RSS value in bytes.
    pub peak_rss_bytes: u64,
    /// Peak VSZ value in bytes.
    pub peak_vsz_bytes: u64,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Additional metadata (platform, architecture, etc.).
    pub metadata: HashMap<String, String>,
}

impl From<&MonitorResult> for Baseline {
    fn from(result: &MonitorResult) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("platform".to_string(), std::env::consts::OS.to_string());
        metadata.insert("arch".to_string(), std::env::consts::ARCH.to_string());

        if let Some(pid) = result.main_pid {
            metadata.insert("main_pid".to_string(), pid.to_string());
        }

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Utc::now(),
            command: result.command.clone(),
            peak_rss_bytes: result.peak_rss_bytes,
            peak_vsz_bytes: result.peak_vsz_bytes,
            duration_ms: result.duration_ms,
            metadata,
        }
    }
}

/// Result of comparing current measurements against a baseline.
///
/// Contains detailed information about differences in memory usage
/// and whether a regression was detected based on the threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// The baseline being compared against.
    pub baseline: Baseline,
    /// Current measurement results.
    pub current: MonitorResult,
    /// Difference in RSS bytes (positive means increase).
    pub rss_diff_bytes: i64,
    /// Percentage change in RSS.
    pub rss_diff_percent: f64,
    /// Difference in VSZ bytes (positive means increase).
    pub vsz_diff_bytes: i64,
    /// Percentage change in VSZ.
    pub vsz_diff_percent: f64,
    /// Difference in duration milliseconds.
    pub duration_diff_ms: i64,
    /// Percentage change in duration.
    pub duration_diff_percent: f64,
    /// Whether memory usage exceeded the regression threshold.
    pub regression_detected: bool,
}

impl ComparisonResult {
    /// Creates a new comparison result.
    ///
    /// # Arguments
    /// * `baseline` - The baseline to compare against
    /// * `current` - Current measurement results
    /// * `threshold_percent` - Percentage increase that triggers regression
    ///   detection
    pub fn new(baseline: Baseline, current: MonitorResult, threshold_percent: f64) -> Self {
        let rss_diff_bytes = current.peak_rss_bytes as i64 - baseline.peak_rss_bytes as i64;
        let rss_diff_percent = if baseline.peak_rss_bytes > 0 {
            (rss_diff_bytes as f64 / baseline.peak_rss_bytes as f64) * 100.0
        } else {
            0.0
        };

        let vsz_diff_bytes = current.peak_vsz_bytes as i64 - baseline.peak_vsz_bytes as i64;
        let vsz_diff_percent = if baseline.peak_vsz_bytes > 0 {
            (vsz_diff_bytes as f64 / baseline.peak_vsz_bytes as f64) * 100.0
        } else {
            0.0
        };

        let duration_diff_ms = current.duration_ms as i64 - baseline.duration_ms as i64;
        let duration_diff_percent = if baseline.duration_ms > 0 {
            (duration_diff_ms as f64 / baseline.duration_ms as f64) * 100.0
        } else {
            0.0
        };

        let regression_detected = rss_diff_percent > threshold_percent;

        Self {
            baseline,
            current,
            rss_diff_bytes,
            rss_diff_percent,
            vsz_diff_bytes,
            vsz_diff_percent,
            duration_diff_ms,
            duration_diff_percent,
            regression_detected,
        }
    }
}

/// Manages baseline storage and retrieval.
///
/// Handles saving baselines to disk, loading them for comparison,
/// and managing the baseline directory.
pub struct BaselineManager {
    baselines_dir: PathBuf,
}

impl BaselineManager {
    /// Creates a new baseline manager with a specific directory.
    ///
    /// # Arguments
    /// * `baselines_dir` - Directory to store baseline files
    ///
    /// # Errors
    /// * Returns error if directory creation fails
    pub fn new(baselines_dir: PathBuf) -> Result<Self> {
        if !baselines_dir.exists() {
            fs::create_dir_all(&baselines_dir)?;
        }
        Ok(Self { baselines_dir })
    }

    /// Returns the default baseline directory path.
    ///
    /// Uses the system cache directory if available, otherwise
    /// falls back to a local directory.
    pub fn default_dir() -> PathBuf {
        if let Some(cache_dir) = dirs::cache_dir() {
            cache_dir.join("peak-mem").join("baselines")
        } else {
            PathBuf::from(".peak-mem-baselines")
        }
    }

    /// Saves a monitoring result as a baseline.
    ///
    /// # Arguments
    /// * `name` - Name for the baseline (will be sanitized)
    /// * `result` - Monitoring results to save
    ///
    /// # Returns
    /// * Path to the saved baseline file
    pub fn save_baseline(&self, name: &str, result: &MonitorResult) -> Result<PathBuf> {
        let baseline = Baseline::from(result);
        let filename = format!("{}.json", sanitize_filename(name));
        let path = self.baselines_dir.join(&filename);

        let json = serde_json::to_string_pretty(&baseline)
            .map_err(|e| PeakMemError::Io(std::io::Error::other(e)))?;

        fs::write(&path, json)?;
        Ok(path)
    }

    pub fn load_baseline(&self, name: &str) -> Result<Baseline> {
        let filename = format!("{}.json", sanitize_filename(name));
        let path = self.baselines_dir.join(&filename);

        let json = fs::read_to_string(&path)?;
        let baseline: Baseline = serde_json::from_str(&json)
            .map_err(|e| PeakMemError::Parse(format!("Failed to parse baseline: {e}")))?;

        Ok(baseline)
    }

    pub fn list_baselines(&self) -> Result<Vec<String>> {
        let mut baselines = Vec::new();

        for entry in fs::read_dir(&self.baselines_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    baselines.push(stem.to_string());
                }
            }
        }

        baselines.sort();
        Ok(baselines)
    }

    pub fn delete_baseline(&self, name: &str) -> Result<()> {
        let filename = format!("{}.json", sanitize_filename(name));
        let path = self.baselines_dir.join(&filename);
        fs::remove_file(&path)?;
        Ok(())
    }

    pub fn compare(
        &self,
        baseline_name: &str,
        current: &MonitorResult,
        threshold_percent: f64,
    ) -> Result<ComparisonResult> {
        let baseline = self.load_baseline(baseline_name)?;
        // Clone is necessary here because ComparisonResult needs to own the
        // MonitorResult for serialization and output formatting purposes
        Ok(ComparisonResult::new(
            baseline,
            current.clone(),
            threshold_percent,
        ))
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_baseline_conversion() {
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
            main_pid: Some(1234),
        };

        let baseline = Baseline::from(&result);
        assert_eq!(baseline.command, "test");
        assert_eq!(baseline.peak_rss_bytes, 100 * 1024 * 1024);
        assert_eq!(baseline.peak_vsz_bytes, 200 * 1024 * 1024);
        assert_eq!(baseline.duration_ms, 5000);
        assert!(baseline.metadata.contains_key("platform"));
        assert!(baseline.metadata.contains_key("arch"));
        assert_eq!(baseline.metadata.get("main_pid"), Some(&"1234".to_string()));
    }

    #[test]
    fn test_comparison_result() {
        let baseline = Baseline {
            version: "0.1.0".to_string(),
            created_at: Utc::now(),
            command: "test".to_string(),
            peak_rss_bytes: 100 * 1024 * 1024,
            peak_vsz_bytes: 200 * 1024 * 1024,
            duration_ms: 5000,
            metadata: HashMap::new(),
        };

        let current = MonitorResult {
            command: "test".to_string(),
            peak_rss_bytes: 110 * 1024 * 1024,
            peak_vsz_bytes: 220 * 1024 * 1024,
            duration_ms: 5500,
            exit_code: Some(0),
            threshold_exceeded: false,
            timestamp: Utc::now(),
            process_tree: None,
            timeline: None,
            start_time: None,
            sample_count: None,
            main_pid: None,
        };

        let comparison = ComparisonResult::new(baseline, current, 5.0);
        assert_eq!(comparison.rss_diff_bytes, 10 * 1024 * 1024);
        assert_eq!(comparison.rss_diff_percent, 10.0);
        assert_eq!(comparison.vsz_diff_bytes, 20 * 1024 * 1024);
        assert_eq!(comparison.vsz_diff_percent, 10.0);
        assert_eq!(comparison.duration_diff_ms, 500);
        assert_eq!(comparison.duration_diff_percent, 10.0);
        assert!(comparison.regression_detected); // 10% > 5% threshold
    }

    #[test]
    fn test_baseline_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BaselineManager::new(temp_dir.path().to_path_buf()).unwrap();

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

        // Save baseline
        let path = manager.save_baseline("test_baseline", &result).unwrap();
        assert!(path.exists());

        // Load baseline
        let loaded = manager.load_baseline("test_baseline").unwrap();
        assert_eq!(loaded.command, "test");
        assert_eq!(loaded.peak_rss_bytes, 100 * 1024 * 1024);

        // List baselines
        let baselines = manager.list_baselines().unwrap();
        assert_eq!(baselines, vec!["test_baseline"]);

        // Delete baseline
        manager.delete_baseline("test_baseline").unwrap();
        let baselines = manager.list_baselines().unwrap();
        assert!(baselines.is_empty());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test/file"), "test_file");
        assert_eq!(sanitize_filename("test:file"), "test_file");
        assert_eq!(sanitize_filename("test*file"), "test_file");
        assert_eq!(sanitize_filename("normal_file"), "normal_file");
    }
}
