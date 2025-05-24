use crate::monitor::{MemoryMonitor, SharedMonitor};
use crate::types::{MemoryUsage, Result};
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

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval_ms));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            // Sample immediately
            let monitor_guard = monitor.lock().await;
            let memory_result = if track_children {
                Self::get_tree_memory(&**monitor_guard, pid).await
            } else {
                monitor_guard.get_memory_usage(pid).await
            };
            drop(monitor_guard);

            if let Ok(usage) = memory_result {
                peak_rss.store(usage.rss_bytes, Ordering::SeqCst);
                peak_vsz.store(usage.vsz_bytes, Ordering::SeqCst);

                let mut tl = timeline.write().await;
                tl.push(usage);
            }

            while running.load(Ordering::SeqCst) {
                interval.tick().await;

                let monitor = monitor.lock().await;
                let memory_result = if track_children {
                    Self::get_tree_memory(&**monitor, pid).await
                } else {
                    monitor.get_memory_usage(pid).await
                };
                drop(monitor);

                match memory_result {
                    Ok(usage) => {
                        // Update peaks
                        peak_rss.fetch_max(usage.rss_bytes, Ordering::SeqCst);
                        peak_vsz.fetch_max(usage.vsz_bytes, Ordering::SeqCst);

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

    async fn get_tree_memory(monitor: &dyn MemoryMonitor, pid: u32) -> Result<MemoryUsage> {
        let tree = monitor.get_process_tree(pid).await?;

        let mut total_rss = 0u64;
        let mut total_vsz = 0u64;

        Self::sum_tree_memory(&tree, &mut total_rss, &mut total_vsz);

        Ok(MemoryUsage {
            rss_bytes: total_rss,
            vsz_bytes: total_vsz,
            timestamp: tree.memory.timestamp,
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
}
