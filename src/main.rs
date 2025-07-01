mod baseline;
mod cli;
mod monitor;
mod output;
mod process;
mod types;

use anyhow::Result;
use baseline::BaselineManager;
use bytesize::ByteSize;
use clap::Parser;
use monitor::tracker::MemoryTracker;
use output::{OutputFormatter, RealtimeDisplay};
use std::time::Instant;
use tokio::time;

/// Application state and logic handler.
struct Application {
    args: cli::Cli,
    baseline_manager: BaselineManager,
}

impl Application {
    /// Creates a new application instance.
    fn new(args: cli::Cli) -> Result<Self> {
        let baseline_dir = args
            .baseline_dir
            .clone()
            .unwrap_or_else(BaselineManager::default_dir);
        let baseline_manager = BaselineManager::new(baseline_dir)?;

        Ok(Self {
            args,
            baseline_manager,
        })
    }

    /// Runs the application.
    async fn run(self) -> Result<()> {
        // Handle baseline-only operations
        if self.handle_baseline_only_operations()? {
            return Ok(());
        }

        // Run the command and monitor memory
        let result = self.monitor_command().await?;

        // Handle output and exit
        self.handle_results(result)
    }

    /// Handles baseline operations that don't require running a command.
    /// Returns true if the operation was handled and the app should exit.
    fn handle_baseline_only_operations(&self) -> Result<bool> {
        if self.args.list_baselines {
            self.list_baselines()?;
            return Ok(true);
        }

        if let Some(name) = &self.args.delete_baseline {
            self.baseline_manager.delete_baseline(name)?;
            println!("Baseline '{name}' deleted.");
            return Ok(true);
        }

        Ok(false)
    }

    /// Lists all saved baselines.
    fn list_baselines(&self) -> Result<()> {
        let baselines = self.baseline_manager.list_baselines()?;
        if baselines.is_empty() {
            println!("No baselines found.");
        } else {
            println!("Saved baselines:");
            for name in baselines {
                println!("  {name}");
            }
        }
        Ok(())
    }

    /// Monitors a command's memory usage.
    async fn monitor_command(&self) -> Result<types::MonitorResult> {
        // Create process runner
        let runner = process::ProcessRunner::new(self.args.command.clone())?;
        let command_string = runner.command_string();

        // Spawn the process
        let handle = runner.spawn().await?;
        let pid = handle.pid();

        // Set up memory tracking
        let monitor = monitor::create_monitor()?;
        let tracker = MemoryTracker::new(monitor, pid, !self.args.no_children);
        let start_time = Instant::now();
        let start_timestamp = chrono::Utc::now();
        let tracker_handle = tracker.start(self.args.interval).await;

        // Run process with optional real-time display
        let exit_code = if self.args.watch {
            run_with_realtime_display(handle, &tracker, self.args.interval).await?
        } else {
            handle.wait_with_signal_forwarding().await?
        };

        // Stop tracking and collect results
        tracker.stop();
        tracker_handle.await?;

        // Build the result
        self.build_monitor_result(
            command_string,
            &tracker,
            start_time,
            start_timestamp,
            exit_code,
            pid,
        )
        .await
    }

    /// Builds the monitoring result from collected data.
    async fn build_monitor_result(
        &self,
        command: String,
        tracker: &MemoryTracker,
        start_time: Instant,
        start_timestamp: chrono::DateTime<chrono::Utc>,
        exit_code: Option<i32>,
        pid: u32,
    ) -> Result<types::MonitorResult> {
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let peak_rss_bytes = tracker.peak_rss();
        let peak_vsz_bytes = tracker.peak_vsz();

        // Check threshold
        let threshold_exceeded = self.check_threshold(peak_rss_bytes);

        // Get optional data based on flags
        let process_tree = self.get_process_tree_if_verbose(tracker).await;
        let timeline = self.get_timeline_if_requested(tracker).await;
        let (start_time_opt, sample_count, main_pid) =
            self.get_verbose_data(start_timestamp, tracker.sample_count(), pid);

        Ok(types::MonitorResult {
            command,
            peak_rss_bytes,
            peak_vsz_bytes,
            duration_ms,
            exit_code,
            threshold_exceeded,
            timestamp: chrono::Utc::now(),
            process_tree,
            timeline,
            start_time: start_time_opt,
            sample_count,
            main_pid,
        })
    }

    /// Checks if the memory usage exceeded the configured threshold.
    fn check_threshold(&self, peak_rss_bytes: u64) -> bool {
        self.args
            .threshold
            .map(|threshold| ByteSize::b(peak_rss_bytes) > threshold)
            .unwrap_or(false)
    }

    /// Gets the process tree if verbose mode is enabled.
    async fn get_process_tree_if_verbose(
        &self,
        tracker: &MemoryTracker,
    ) -> Option<types::ProcessMemoryInfo> {
        if self.args.verbose && !self.args.no_children {
            match tracker.get_process_tree().await {
                Ok(tree) => Some(tree),
                Err(e) => {
                    eprintln!("Warning: Failed to get process tree: {e}");
                    None
                }
            }
        } else {
            None
        }
    }

    /// Gets the timeline if requested.
    async fn get_timeline_if_requested(
        &self,
        tracker: &MemoryTracker,
    ) -> Option<Vec<types::MemoryUsage>> {
        if self.args.timeline.is_some() {
            Some(tracker.timeline().await)
        } else {
            None
        }
    }

    /// Gets verbose data if verbose mode is enabled.
    fn get_verbose_data(
        &self,
        start_timestamp: chrono::DateTime<chrono::Utc>,
        sample_count: u64,
        pid: u32,
    ) -> (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<u64>,
        Option<u32>,
    ) {
        if self.args.verbose {
            (Some(start_timestamp), Some(sample_count), Some(pid))
        } else {
            (None, None, None)
        }
    }

    /// Handles the results: saves timeline, manages baselines, formats output.
    fn handle_results(&self, result: types::MonitorResult) -> Result<()> {
        // Save timeline if requested
        if let Err(e) = self.save_timeline_if_requested(&result) {
            eprintln!("Warning: Failed to save timeline: {e}");
        }

        // Handle baseline operations
        self.handle_baseline_operations(&result)?;

        // Handle comparison or normal output
        let exit_code = if let Some(baseline_name) = &self.args.compare_baseline {
            self.handle_comparison(baseline_name, &result)?
        } else {
            self.handle_normal_output(&result)?
        };

        // Exit with appropriate code
        if let Some(code) = exit_code {
            std::process::exit(code);
        }

        Ok(())
    }

    /// Saves the timeline to a file if requested.
    fn save_timeline_if_requested(&self, result: &types::MonitorResult) -> Result<()> {
        if let Some(timeline_path) = &self.args.timeline {
            if let Some(timeline) = &result.timeline {
                let json = serde_json::to_string_pretty(timeline)?;
                std::fs::write(timeline_path, json)?;
            }
        }
        Ok(())
    }

    /// Handles baseline save operations.
    fn handle_baseline_operations(&self, result: &types::MonitorResult) -> Result<()> {
        if let Some(baseline_name) = &self.args.save_baseline {
            let path = self.baseline_manager.save_baseline(baseline_name, result)?;
            eprintln!("Baseline '{}' saved to: {}", baseline_name, path.display());
        }
        Ok(())
    }

    /// Handles baseline comparison.
    fn handle_comparison(
        &self,
        baseline_name: &str,
        result: &types::MonitorResult,
    ) -> Result<Option<i32>> {
        let comparison =
            self.baseline_manager
                .compare(baseline_name, result, self.args.regression_threshold)?;
        OutputFormatter::format_comparison(&comparison, self.args.output_format())?;

        if comparison.regression_detected {
            Ok(Some(1))
        } else {
            Ok(result.exit_code)
        }
    }

    /// Handles normal output (no comparison).
    fn handle_normal_output(&self, result: &types::MonitorResult) -> Result<Option<i32>> {
        OutputFormatter::format(result, self.args.output_format(), self.args.verbose)?;

        if result.threshold_exceeded {
            Ok(Some(1))
        } else {
            Ok(result.exit_code)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();
    let app = Application::new(args)?;
    app.run().await
}

async fn run_with_realtime_display(
    handle: process::ProcessHandle,
    tracker: &MemoryTracker,
    interval_ms: u64,
) -> Result<Option<i32>> {
    let pid = handle.pid();
    let monitor = monitor::create_monitor()?;
    let peak_rss_atom = tracker.peak_rss.clone();
    let peak_vsz_atom = tracker.peak_vsz.clone();

    let monitor_task = tokio::spawn(async move {
        let mut display = RealtimeDisplay::new();
        let mut interval = time::interval(time::Duration::from_millis(interval_ms));

        loop {
            interval.tick().await;

            if let Ok(usage) = monitor.get_memory_usage(pid).await {
                let current_rss = ByteSize::b(usage.rss_bytes);
                let current_vsz = ByteSize::b(usage.vsz_bytes);
                let peak_rss = ByteSize::b(peak_rss_atom.load(std::sync::atomic::Ordering::SeqCst));
                let peak_vsz = ByteSize::b(peak_vsz_atom.load(std::sync::atomic::Ordering::SeqCst));

                if display
                    .update(current_rss, peak_rss, current_vsz, peak_vsz)
                    .is_err()
                {
                    break;
                }
            } else {
                // Process terminated
                break;
            }
        }

        let _ = display.clear();
    });

    let exit_code = handle.wait_with_signal_forwarding().await?;
    monitor_task.abort();

    Ok(exit_code)
}
