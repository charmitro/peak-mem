mod cli;
mod monitor;
mod output;
mod process;
mod types;

use anyhow::Result;
use bytesize::ByteSize;
use clap::Parser;
use monitor::tracker::MemoryTracker;
use output::{OutputFormatter, RealtimeDisplay};
use std::time::Instant;
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();

    let runner = process::ProcessRunner::new(args.command.clone())?;
    let command_string = runner.command_string();

    // Spawn the process
    let handle = runner.spawn().await?;
    let pid = handle.pid();

    // Create memory monitor
    let monitor = monitor::create_monitor()?;
    let tracker = MemoryTracker::new(monitor, pid, !args.no_children);

    // Start tracking
    let start_time = Instant::now();
    let start_timestamp = chrono::Utc::now();
    let tracker_handle = tracker.start(args.interval).await;

    // Handle real-time display if requested
    let exit_code = if args.watch {
        run_with_realtime_display(handle, &tracker, args.interval).await?
    } else {
        handle.wait_with_signal_forwarding().await?
    };

    // Stop tracking
    tracker.stop();
    tracker_handle.await?;

    // Collect results
    let duration_ms = start_time.elapsed().as_millis() as u64;
    let peak_rss_bytes = tracker.peak_rss();
    let peak_vsz_bytes = tracker.peak_vsz();

    // Check threshold
    let threshold_exceeded = if let Some(threshold) = args.threshold {
        ByteSize::b(peak_rss_bytes) > threshold
    } else {
        false
    };

    // Get process tree for verbose mode
    let process_tree = if args.verbose && !args.no_children {
        match tracker.get_process_tree().await {
            Ok(tree) => Some(tree),
            Err(e) => {
                eprintln!("Warning: Failed to get process tree: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Build result
    let result = types::MonitorResult {
        command: command_string,
        peak_rss_bytes,
        peak_vsz_bytes,
        duration_ms,
        exit_code,
        threshold_exceeded,
        timestamp: chrono::Utc::now(),
        process_tree,
        timeline: if args.timeline.is_some() {
            Some(tracker.timeline().await)
        } else {
            None
        },
        start_time: if args.verbose {
            Some(start_timestamp)
        } else {
            None
        },
        sample_count: if args.verbose {
            Some(tracker.sample_count())
        } else {
            None
        },
        main_pid: if args.verbose { Some(pid) } else { None },
    };

    // Save timeline if requested
    if let Some(timeline_path) = &args.timeline {
        if let Some(timeline) = &result.timeline {
            let json = serde_json::to_string_pretty(timeline)?;
            tokio::fs::write(timeline_path, json).await?;
        }
    }

    // Format output
    OutputFormatter::format(&result, args.output_format(), args.verbose)?;

    // Exit with appropriate code
    if threshold_exceeded {
        std::process::exit(1);
    } else if let Some(code) = exit_code {
        std::process::exit(code);
    }

    Ok(())
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
