use crate::baseline::ComparisonResult;
use crate::cli::OutputFormat;
use crate::types::{MonitorResult, ProcessMemoryInfo};
use anyhow::Result;
use bytesize::ByteSize;
use std::io::{self, Write};

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn format(result: &MonitorResult, format: OutputFormat, verbose: bool) -> Result<()> {
        match format {
            OutputFormat::Human => {
                if verbose {
                    Self::format_verbose(result)
                } else {
                    Self::format_human(result)
                }
            }
            OutputFormat::Json => Self::format_json(result),
            OutputFormat::Csv => Self::format_csv(result),
            OutputFormat::Quiet => Self::format_quiet(result),
        }
    }

    fn format_human(result: &MonitorResult) -> Result<()> {
        let mut stdout = io::stdout();

        writeln!(stdout, "Command: {}", result.command)?;
        write!(stdout, "Peak memory usage: {} (RSS)", result.peak_rss())?;
        writeln!(stdout, " / {} (VSZ)", result.peak_vsz())?;

        if let Some(exit_code) = result.exit_code {
            writeln!(stdout, "Exit code: {}", exit_code)?;
        }

        writeln!(stdout, "Duration: {:.1}s", result.duration().as_secs_f64())?;

        if result.threshold_exceeded {
            writeln!(stdout, "\n⚠️  THRESHOLD EXCEEDED")?;
        }

        stdout.flush()?;
        Ok(())
    }

    fn format_json(result: &MonitorResult) -> Result<()> {
        let json = serde_json::to_string_pretty(result)?;
        println!("{}", json);
        Ok(())
    }

    fn format_csv(result: &MonitorResult) -> Result<()> {
        let mut wtr = csv::Writer::from_writer(io::stdout());

        wtr.write_record([
            "command",
            "peak_rss_bytes",
            "peak_vsz_bytes",
            "duration_ms",
            "exit_code",
            "threshold_exceeded",
            "timestamp",
        ])?;

        wtr.write_record([
            &result.command,
            &result.peak_rss_bytes.to_string(),
            &result.peak_vsz_bytes.to_string(),
            &result.duration_ms.to_string(),
            &result.exit_code.map_or("".to_string(), |c| c.to_string()),
            &result.threshold_exceeded.to_string(),
            &result.timestamp.to_rfc3339(),
        ])?;

        wtr.flush()?;
        Ok(())
    }

    fn format_quiet(result: &MonitorResult) -> Result<()> {
        println!("{}", result.peak_rss_bytes);
        Ok(())
    }

    fn format_verbose(result: &MonitorResult) -> Result<()> {
        let mut stdout = io::stdout();

        // Header
        writeln!(stdout, "Command: {}", result.command)?;
        if let Some(start_time) = result.start_time {
            writeln!(
                stdout,
                "Started: {} UTC",
                start_time.format("%Y-%m-%d %H:%M:%S")
            )?;
        }
        if let Some(pid) = result.main_pid {
            writeln!(stdout, "Process ID: {}", pid)?;
        }
        writeln!(stdout)?;

        // Memory Usage Section
        writeln!(stdout, "Memory Usage:")?;
        writeln!(
            stdout,
            "  Peak RSS: {} ({} bytes)",
            result.peak_rss(),
            result.peak_rss_bytes
        )?;
        writeln!(
            stdout,
            "  Peak VSZ: {} ({} bytes)",
            result.peak_vsz(),
            result.peak_vsz_bytes
        )?;
        writeln!(stdout)?;

        // Process Tree Section
        if let Some(tree) = &result.process_tree {
            let process_count = Self::count_processes(tree);
            writeln!(
                stdout,
                "Process Tree: ({} processes monitored)",
                process_count
            )?;
            Self::print_process_tree(&mut stdout, tree, "", true)?;
        } else {
            writeln!(
                stdout,
                "Process Tree: (monitoring disabled with --no-children)"
            )?;
        }
        writeln!(stdout)?;

        // Performance Section
        writeln!(stdout, "Performance:")?;
        writeln!(
            stdout,
            "  Duration: {:.3}s",
            result.duration().as_secs_f64()
        )?;
        if let Some(sample_count) = result.sample_count {
            writeln!(stdout, "  Samples collected: {}", sample_count)?;
        }
        writeln!(
            stdout,
            "  Sampling interval: {}ms",
            result.duration_ms / result.sample_count.unwrap_or(1).max(1)
        )?;
        writeln!(stdout)?;

        // Exit Status
        if let Some(exit_code) = result.exit_code {
            writeln!(
                stdout,
                "Exit Status: {} ({})",
                exit_code,
                if exit_code == 0 { "success" } else { "failed" }
            )?;
        }

        // Threshold Status
        if result.threshold_exceeded {
            writeln!(stdout, "\n⚠️  THRESHOLD EXCEEDED")?;
        }

        stdout.flush()?;
        Ok(())
    }

    fn count_processes(tree: &ProcessMemoryInfo) -> usize {
        1 + tree
            .children
            .iter()
            .map(Self::count_processes)
            .sum::<usize>()
    }

    fn print_process_tree(
        stdout: &mut dyn Write,
        tree: &ProcessMemoryInfo,
        prefix: &str,
        is_last: bool,
    ) -> Result<()> {
        // Print current process
        let connector = if is_last { "└── " } else { "├── " };
        let name = if tree.name.len() > 40 {
            format!("{}...", &tree.name[..37])
        } else {
            tree.name.clone()
        };

        writeln!(
            stdout,
            "{}{}{} (PID: {}) - Peak: {}",
            prefix,
            if prefix.is_empty() { "" } else { connector },
            name,
            tree.pid,
            ByteSize::b(tree.memory.rss_bytes)
        )?;

        // Sort children by peak RSS (descending)
        let mut children = tree.children.clone();
        children.sort_by(|a, b| b.memory.rss_bytes.cmp(&a.memory.rss_bytes));

        // Print children with proper tree structure
        let child_prefix = format!(
            "{}{}",
            prefix,
            if prefix.is_empty() {
                ""
            } else if is_last {
                "    "
            } else {
                "│   "
            }
        );

        for (i, child) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            Self::print_process_tree(stdout, child, &child_prefix, is_last_child)?;
        }

        Ok(())
    }

    pub fn format_comparison(comparison: &ComparisonResult, format: OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Human => Self::format_comparison_human(comparison),
            OutputFormat::Json => Self::format_comparison_json(comparison),
            OutputFormat::Csv => Self::format_comparison_csv(comparison),
            OutputFormat::Quiet => Self::format_comparison_quiet(comparison),
        }
    }

    fn format_comparison_human(comparison: &ComparisonResult) -> Result<()> {
        let mut stdout = io::stdout();

        writeln!(stdout, "Command: {}", comparison.current.command)?;
        writeln!(stdout)?;

        writeln!(stdout, "Baseline vs Current:")?;
        writeln!(
            stdout,
            "  Peak RSS: {} → {} ({:+.1}%)",
            ByteSize::b(comparison.baseline.peak_rss_bytes),
            comparison.current.peak_rss(),
            comparison.rss_diff_percent
        )?;

        if comparison.rss_diff_bytes > 0 {
            writeln!(
                stdout,
                "  Absolute increase: {}",
                ByteSize::b(comparison.rss_diff_bytes as u64)
            )?;
        } else if comparison.rss_diff_bytes < 0 {
            writeln!(
                stdout,
                "  Absolute decrease: {}",
                ByteSize::b((-comparison.rss_diff_bytes) as u64)
            )?;
        }

        writeln!(stdout)?;
        writeln!(
            stdout,
            "  Peak VSZ: {} → {} ({:+.1}%)",
            ByteSize::b(comparison.baseline.peak_vsz_bytes),
            comparison.current.peak_vsz(),
            comparison.vsz_diff_percent
        )?;

        writeln!(stdout)?;
        writeln!(
            stdout,
            "  Duration: {:.1}s → {:.1}s ({:+.1}%)",
            comparison.baseline.duration_ms as f64 / 1000.0,
            comparison.current.duration().as_secs_f64(),
            comparison.duration_diff_percent
        )?;

        writeln!(stdout)?;
        if comparison.regression_detected {
            writeln!(
                stdout,
                "❌ REGRESSION DETECTED: Memory usage increased by {:.1}%",
                comparison.rss_diff_percent
            )?;
        } else {
            writeln!(stdout, "✅ No regression detected")?;
        }

        stdout.flush()?;
        Ok(())
    }

    fn format_comparison_json(comparison: &ComparisonResult) -> Result<()> {
        let json = serde_json::to_string_pretty(comparison)?;
        println!("{}", json);
        Ok(())
    }

    fn format_comparison_csv(comparison: &ComparisonResult) -> Result<()> {
        let mut wtr = csv::Writer::from_writer(io::stdout());

        wtr.write_record([
            "baseline_command",
            "baseline_rss_bytes",
            "baseline_vsz_bytes",
            "baseline_duration_ms",
            "current_command",
            "current_rss_bytes",
            "current_vsz_bytes",
            "current_duration_ms",
            "rss_diff_bytes",
            "rss_diff_percent",
            "vsz_diff_bytes",
            "vsz_diff_percent",
            "duration_diff_ms",
            "duration_diff_percent",
            "regression_detected",
        ])?;

        wtr.write_record([
            &comparison.baseline.command,
            &comparison.baseline.peak_rss_bytes.to_string(),
            &comparison.baseline.peak_vsz_bytes.to_string(),
            &comparison.baseline.duration_ms.to_string(),
            &comparison.current.command,
            &comparison.current.peak_rss_bytes.to_string(),
            &comparison.current.peak_vsz_bytes.to_string(),
            &comparison.current.duration_ms.to_string(),
            &comparison.rss_diff_bytes.to_string(),
            &comparison.rss_diff_percent.to_string(),
            &comparison.vsz_diff_bytes.to_string(),
            &comparison.vsz_diff_percent.to_string(),
            &comparison.duration_diff_ms.to_string(),
            &comparison.duration_diff_percent.to_string(),
            &comparison.regression_detected.to_string(),
        ])?;

        wtr.flush()?;
        Ok(())
    }

    fn format_comparison_quiet(comparison: &ComparisonResult) -> Result<()> {
        if comparison.regression_detected {
            println!("regression");
        } else {
            println!("ok");
        }
        Ok(())
    }
}

pub struct RealtimeDisplay {
    last_line_count: usize,
}

impl RealtimeDisplay {
    pub fn new() -> Self {
        Self { last_line_count: 0 }
    }

    pub fn update(
        &mut self,
        current_rss: ByteSize,
        peak_rss: ByteSize,
        current_vsz: ByteSize,
        peak_vsz: ByteSize,
    ) -> Result<()> {
        use crossterm::{cursor, terminal, ExecutableCommand};
        let mut stdout = io::stdout();

        // Clear previous lines
        for _ in 0..self.last_line_count {
            stdout.execute(cursor::MoveToPreviousLine(1))?;
            stdout.execute(terminal::Clear(terminal::ClearType::CurrentLine))?;
        }

        // Print new status
        writeln!(
            stdout,
            "Current RSS: {} | Peak RSS: {}",
            current_rss, peak_rss
        )?;
        writeln!(
            stdout,
            "Current VSZ: {} | Peak VSZ: {}",
            current_vsz, peak_vsz
        )?;
        stdout.flush()?;

        self.last_line_count = 2;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        use crossterm::{cursor, terminal, ExecutableCommand};
        let mut stdout = io::stdout();

        for _ in 0..self.last_line_count {
            stdout.execute(cursor::MoveToPreviousLine(1))?;
            stdout.execute(terminal::Clear(terminal::ClearType::CurrentLine))?;
        }
        stdout.flush()?;
        self.last_line_count = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MemoryUsage;
    use chrono::Utc;

    #[test]
    fn test_format_quiet() {
        let result = MonitorResult {
            command: "test".to_string(),
            peak_rss_bytes: 123456789,
            peak_vsz_bytes: 987654321,
            duration_ms: 1000,
            exit_code: Some(0),
            threshold_exceeded: false,
            timestamp: Utc::now(),
            process_tree: None,
            timeline: None,
            start_time: None,
            sample_count: None,
            main_pid: None,
        };

        // Quiet format should just print the RSS bytes
        OutputFormatter::format(&result, OutputFormat::Quiet, false).unwrap();
    }

    #[test]
    fn test_format_verbose() {
        let now = Utc::now();

        // Create a sample process tree
        let child_process = ProcessMemoryInfo {
            pid: 12346,
            name: "rustc".to_string(),
            memory: MemoryUsage {
                rss_bytes: 442_123_456,
                vsz_bytes: 512_123_456,
                timestamp: now,
            },
            children: vec![
                ProcessMemoryInfo {
                    pid: 12347,
                    name: "cc".to_string(),
                    memory: MemoryUsage {
                        rss_bytes: 23_456_789,
                        vsz_bytes: 45_678_901,
                        timestamp: now,
                    },
                    children: vec![],
                },
                ProcessMemoryInfo {
                    pid: 12348,
                    name: "ld".to_string(),
                    memory: MemoryUsage {
                        rss_bytes: 89_123_456,
                        vsz_bytes: 123_456_789,
                        timestamp: now,
                    },
                    children: vec![],
                },
            ],
        };

        let root_process = ProcessMemoryInfo {
            pid: 12345,
            name: "cargo".to_string(),
            memory: MemoryUsage {
                rss_bytes: 45_234_567,
                vsz_bytes: 78_901_234,
                timestamp: now,
            },
            children: vec![child_process],
        };

        let result = MonitorResult {
            command: "cargo build --release".to_string(),
            peak_rss_bytes: 487_300_000,
            peak_vsz_bytes: 892_100_000,
            duration_ms: 14_263,
            exit_code: Some(0),
            threshold_exceeded: false,
            timestamp: now,
            process_tree: Some(root_process),
            timeline: None,
            start_time: Some(now),
            sample_count: Some(142),
            main_pid: Some(12345),
        };

        // Test verbose format - should not panic
        OutputFormatter::format(&result, OutputFormat::Human, true).unwrap();
    }

    #[test]
    fn test_format_verbose_no_children() {
        let now = Utc::now();

        let result = MonitorResult {
            command: "echo test".to_string(),
            peak_rss_bytes: 10_485_760,
            peak_vsz_bytes: 20_971_520,
            duration_ms: 100,
            exit_code: Some(0),
            threshold_exceeded: false,
            timestamp: now,
            process_tree: None,
            timeline: None,
            start_time: Some(now),
            sample_count: Some(1),
            main_pid: Some(99999),
        };

        // Test verbose format without process tree
        OutputFormatter::format(&result, OutputFormat::Human, true).unwrap();
    }

    #[test]
    fn test_count_processes() {
        let now = Utc::now();
        let tree = ProcessMemoryInfo {
            pid: 1,
            name: "root".to_string(),
            memory: MemoryUsage {
                rss_bytes: 1000,
                vsz_bytes: 2000,
                timestamp: now,
            },
            children: vec![
                ProcessMemoryInfo {
                    pid: 2,
                    name: "child1".to_string(),
                    memory: MemoryUsage {
                        rss_bytes: 100,
                        vsz_bytes: 200,
                        timestamp: now,
                    },
                    children: vec![],
                },
                ProcessMemoryInfo {
                    pid: 3,
                    name: "child2".to_string(),
                    memory: MemoryUsage {
                        rss_bytes: 200,
                        vsz_bytes: 400,
                        timestamp: now,
                    },
                    children: vec![ProcessMemoryInfo {
                        pid: 4,
                        name: "grandchild".to_string(),
                        memory: MemoryUsage {
                            rss_bytes: 50,
                            vsz_bytes: 100,
                            timestamp: now,
                        },
                        children: vec![],
                    }],
                },
            ],
        };

        assert_eq!(OutputFormatter::count_processes(&tree), 4);
    }
}
