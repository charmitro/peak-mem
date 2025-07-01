//! Continuous memory tracking with peak detection.
//!
//! This module provides the `MemoryTracker` which continuously monitors
//! a process's memory usage and maintains peak values.

use crate::monitor::{MemoryMonitor, SharedMonitor};
use crate::types::{MemoryUsage, ProcessMemoryInfo, Result};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

/// Tracks memory usage over time for a process and its children.
///
/// The tracker runs in a background task, periodically sampling memory usage
/// and updating peak values using lock-free atomic operations.
pub struct MemoryTracker {
    monitor: SharedMonitor,
    pid: u32,
    /// Peak RSS value observed (in bytes), updated atomically.
    pub peak_rss: Arc<AtomicU64>,
    /// Peak VSZ value observed (in bytes), updated atomically.
    pub peak_vsz: Arc<AtomicU64>,
    timeline: Arc<RwLock<Vec<MemoryUsage>>>,
    running: Arc<AtomicBool>,
    track_children: bool,
    sample_count: Arc<AtomicU64>,
    peak_process_tree: Arc<RwLock<Option<ProcessMemoryInfo>>>,
}

impl MemoryTracker {
    /// Creates a new memory tracker for a specific process.
    ///
    /// # Arguments
    /// * `monitor` - Platform-specific memory monitor implementation
    /// * `pid` - Process ID to track
    /// * `track_children` - Whether to include child processes in measurements
    pub fn new(monitor: Box<dyn MemoryMonitor>, pid: u32, track_children: bool) -> Self {
        Self {
            monitor: Arc::new(tokio::sync::Mutex::new(monitor)),
            pid,
            peak_rss: Arc::new(AtomicU64::new(0)),
            peak_vsz: Arc::new(AtomicU64::new(0)),
            timeline: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            track_children,
            sample_count: Arc::new(AtomicU64::new(0)),
            peak_process_tree: Arc::new(RwLock::new(None)),
        }
    }

    /// Starts the background tracking task.
    ///
    /// The task will sample memory usage at the specified interval until
    /// `stop()` is called.
    ///
    /// # Arguments
    /// * `interval_ms` - Sampling interval in milliseconds
    ///
    /// # Returns
    /// * `JoinHandle` for the spawned tracking task
    pub async fn start(&self, interval_ms: u64) -> tokio::task::JoinHandle<()> {
        let monitor = Arc::clone(&self.monitor);
        let pid = self.pid;
        let peak_rss = Arc::clone(&self.peak_rss);
        let peak_vsz = Arc::clone(&self.peak_vsz);
        let timeline = Arc::clone(&self.timeline);
        let running = Arc::clone(&self.running);
        let track_children = self.track_children;
        let sample_count = Arc::clone(&self.sample_count);
        let peak_process_tree = Arc::clone(&self.peak_process_tree);

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval_ms));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            // Sample immediately
            let monitor_guard = monitor.lock().await;
            if track_children {
                if let Ok(tree) = monitor_guard.get_process_tree(pid).await {
                    let mut total_rss = 0u64;
                    let mut total_vsz = 0u64;
                    Self::sum_tree_memory(&tree, &mut total_rss, &mut total_vsz);

                    peak_rss.store(total_rss, Ordering::SeqCst);
                    peak_vsz.store(total_vsz, Ordering::SeqCst);
                    sample_count.fetch_add(1, Ordering::SeqCst);

                    // Store initial process tree
                    let mut pt = peak_process_tree.write().await;
                    *pt = Some(tree.clone());

                    let mut tl = timeline.write().await;
                    tl.push(MemoryUsage {
                        rss_bytes: total_rss,
                        vsz_bytes: total_vsz,
                        timestamp: tree.memory.timestamp,
                    });
                }
            } else if let Ok(usage) = monitor_guard.get_memory_usage(pid).await {
                peak_rss.store(usage.rss_bytes, Ordering::SeqCst);
                peak_vsz.store(usage.vsz_bytes, Ordering::SeqCst);
                sample_count.fetch_add(1, Ordering::SeqCst);

                let mut tl = timeline.write().await;
                tl.push(usage);
            }
            drop(monitor_guard);

            while running.load(Ordering::SeqCst) {
                interval.tick().await;

                let monitor = monitor.lock().await;
                if track_children {
                    match monitor.get_process_tree(pid).await {
                        Ok(tree) => {
                            let mut total_rss = 0u64;
                            let mut total_vsz = 0u64;
                            Self::sum_tree_memory(&tree, &mut total_rss, &mut total_vsz);

                            // Check if this is a new peak
                            let old_peak = peak_rss.load(Ordering::SeqCst);
                            if total_rss > old_peak {
                                peak_rss.store(total_rss, Ordering::SeqCst);
                                peak_vsz.store(total_vsz, Ordering::SeqCst);

                                // Update peak process tree
                                let mut pt = peak_process_tree.write().await;
                                *pt = Some(tree.clone());
                            } else {
                                peak_rss.fetch_max(total_rss, Ordering::SeqCst);
                                peak_vsz.fetch_max(total_vsz, Ordering::SeqCst);
                            }

                            sample_count.fetch_add(1, Ordering::SeqCst);

                            let mut tl = timeline.write().await;
                            tl.push(MemoryUsage {
                                rss_bytes: total_rss,
                                vsz_bytes: total_vsz,
                                timestamp: tree.memory.timestamp,
                            });
                        }
                        Err(_) => {
                            // Process likely terminated
                            break;
                        }
                    }
                } else {
                    match monitor.get_memory_usage(pid).await {
                        Ok(usage) => {
                            // Update peaks
                            peak_rss.fetch_max(usage.rss_bytes, Ordering::SeqCst);
                            peak_vsz.fetch_max(usage.vsz_bytes, Ordering::SeqCst);
                            sample_count.fetch_add(1, Ordering::SeqCst);

                            // Add to timeline
                            let mut tl = timeline.write().await;
                            tl.push(usage);
                        }
                        Err(_) => {
                            // Process likely terminated
                            break;
                        }
                    }
                }
                drop(monitor);
            }
        })
    }

    /// Stops the background tracking task.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns the peak RSS value observed so far.
    pub fn peak_rss(&self) -> u64 {
        self.peak_rss.load(Ordering::SeqCst)
    }

    /// Returns the peak VSZ value observed so far.
    pub fn peak_vsz(&self) -> u64 {
        self.peak_vsz.load(Ordering::SeqCst)
    }

    /// Returns a copy of the collected timeline data.
    pub async fn timeline(&self) -> Vec<MemoryUsage> {
        self.timeline.read().await.clone()
    }

    /// Returns the number of samples collected.
    pub fn sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::SeqCst)
    }

    /// Returns the process tree captured at peak memory usage.
    ///
    /// # Returns
    /// * `Ok(ProcessMemoryInfo)` - Process tree at peak
    /// * `Err` - If no process tree has been captured yet
    pub async fn get_process_tree(&self) -> Result<crate::types::ProcessMemoryInfo> {
        let tree_lock = self.peak_process_tree.read().await;
        tree_lock.clone().ok_or_else(|| {
            crate::types::PeakMemError::ProcessSpawn("No process tree available".to_string())
        })
    }

    /// Recursively sums memory usage across a process tree.
    ///
    /// # Arguments
    /// * `info` - Root of process tree
    /// * `rss` - Accumulator for RSS bytes
    /// * `vsz` - Accumulator for VSZ bytes
    fn sum_tree_memory(info: &crate::types::ProcessMemoryInfo, rss: &mut u64, vsz: &mut u64) {
        *rss += info.memory.rss_bytes;
        *vsz += info.memory.vsz_bytes;

        for child in &info.children {
            Self::sum_tree_memory(child, rss, vsz);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::create_monitor;

    #[tokio::test]
    async fn test_memory_tracker() {
        let monitor = create_monitor().unwrap();
        let pid = std::process::id();
        let tracker = MemoryTracker::new(monitor, pid, false);

        // Start tracking with very short interval
        let handle = tracker.start(1).await;

        // Wait for at least one sample to be collected
        // Instead of time-based wait, check for samples
        let mut retries = 0;
        while tracker.sample_count() == 0 && retries < 100 {
            tokio::task::yield_now().await;
            retries += 1;
        }

        tracker.stop();
        handle.await.unwrap();

        // Verify we collected data
        assert!(tracker.peak_rss() > 0, "Peak RSS should be greater than 0");
        assert!(tracker.peak_vsz() > 0, "Peak VSZ should be greater than 0");
        assert!(
            tracker.sample_count() > 0,
            "Should have collected at least one sample"
        );

        let timeline = tracker.timeline().await;
        assert!(!timeline.is_empty(), "Timeline should not be empty");
    }

    #[tokio::test]
    async fn test_process_tree_capture() {
        let monitor = create_monitor().unwrap();
        let pid = std::process::id();
        let tracker = MemoryTracker::new(monitor, pid, true);

        // Start tracking
        let handle = tracker.start(1).await;

        // Wait for process tree to be captured
        let mut retries = 0;
        let mut tree_captured = false;
        while retries < 100 {
            if tracker.get_process_tree().await.is_ok() {
                tree_captured = true;
                break;
            }
            tokio::task::yield_now().await;
            retries += 1;
        }

        tracker.stop();
        handle.await.unwrap();

        // Verify process tree was captured
        assert!(tree_captured, "Process tree should have been captured");
        let tree = tracker.get_process_tree().await.unwrap();
        assert_eq!(tree.pid, pid);
        assert!(!tree.name.is_empty());
        assert!(tree.memory.rss_bytes > 0);
    }

    #[tokio::test]
    async fn test_process_tree_with_children() {
        use tokio::process::Command;

        // Create a process that will definitely exist long enough to be monitored
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("while true; do sleep 0.1; done")
            .spawn()
            .expect("Failed to spawn test process");

        let pid = child.id().expect("Failed to get PID");

        let monitor = create_monitor().unwrap();
        let tracker = MemoryTracker::new(monitor, pid, true);

        // Start tracking with short interval
        let handle = tracker.start(1).await;

        // Wait for process tree to be captured (deterministic check)
        let mut tree_captured = false;
        let mut retries = 0;
        while retries < 100 {
            if let Ok(tree) = tracker.get_process_tree().await {
                if tree.pid == pid && tree.memory.rss_bytes > 0 {
                    tree_captured = true;
                    break;
                }
            }
            tokio::task::yield_now().await;
            retries += 1;
        }

        tracker.stop();

        // Clean up first
        let _ = child.kill().await;
        let _ = child.wait().await;
        handle.await.unwrap();

        // Now assert
        assert!(tree_captured, "Should have captured process tree");
        assert!(tracker.sample_count() > 0, "Should have collected samples");
    }
}
