use crate::monitor::{MemoryMonitor, SharedMonitor};
use crate::types::{MemoryUsage, ProcessMemoryInfo, Result};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

pub struct MemoryTracker {
    monitor: SharedMonitor,
    pid: u32,
    pub peak_rss: Arc<AtomicU64>,
    pub peak_vsz: Arc<AtomicU64>,
    timeline: Arc<RwLock<Vec<MemoryUsage>>>,
    running: Arc<AtomicBool>,
    track_children: bool,
    sample_count: Arc<AtomicU64>,
    peak_process_tree: Arc<RwLock<Option<ProcessMemoryInfo>>>,
}

impl MemoryTracker {
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

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn peak_rss(&self) -> u64 {
        self.peak_rss.load(Ordering::SeqCst)
    }

    pub fn peak_vsz(&self) -> u64 {
        self.peak_vsz.load(Ordering::SeqCst)
    }

    pub async fn timeline(&self) -> Vec<MemoryUsage> {
        self.timeline.read().await.clone()
    }

    pub fn sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::SeqCst)
    }

    pub async fn get_process_tree(&self) -> Result<crate::types::ProcessMemoryInfo> {
        let tree_lock = self.peak_process_tree.read().await;
        tree_lock.clone().ok_or_else(|| {
            crate::types::PeakMemError::ProcessSpawn("No process tree available".to_string())
        })
    }

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

        let handle = tracker.start(10).await;

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        tracker.stop();
        handle.await.unwrap();

        assert!(tracker.peak_rss() > 0);
        assert!(tracker.peak_vsz() > 0);

        let timeline = tracker.timeline().await;
        assert!(!timeline.is_empty());
    }

    #[tokio::test]
    async fn test_process_tree_capture() {
        let monitor = create_monitor().unwrap();
        let pid = std::process::id();
        let tracker = MemoryTracker::new(monitor, pid, true);

        let handle = tracker.start(10).await;

        // Let it run for a bit to capture tree
        tokio::time::sleep(Duration::from_millis(50)).await;

        tracker.stop();
        handle.await.unwrap();

        // Should have captured process tree
        let tree_result = tracker.get_process_tree().await;
        assert!(tree_result.is_ok());

        let tree = tree_result.unwrap();
        assert_eq!(tree.pid, pid);
        assert!(!tree.name.is_empty());
        assert!(tree.memory.rss_bytes > 0);
    }

    #[tokio::test]
    async fn test_process_tree_with_children() {
        use tokio::process::Command;

        // Spawn a shell with sleep children
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("sleep 1 & sleep 1 & wait")
            .spawn()
            .expect("Failed to spawn test process");

        let pid = child.id().expect("Failed to get PID");

        let monitor = create_monitor().unwrap();
        let tracker = MemoryTracker::new(monitor, pid, true);

        let handle = tracker.start(10).await;

        // Let it run to capture children
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check we captured a tree with children
        let tree_result = tracker.get_process_tree().await;
        assert!(tree_result.is_ok());

        let tree = tree_result.unwrap();
        assert_eq!(tree.pid, pid);

        // Should have at least 2 sleep children
        assert!(
            tree.children.len() >= 2,
            "Expected at least 2 children, got {}",
            tree.children.len()
        );

        tracker.stop();

        // Clean up
        let _ = child.wait().await;
        handle.await.unwrap();
    }
}
