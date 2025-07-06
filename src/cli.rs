use anyhow::Result;
use bytesize::ByteSize;
use clap::{ArgAction, Parser};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Kibibytes,
    Mebibytes,
    Gibibytes,
}

impl MemoryUnit {
    pub fn format(&self, bytes: u64) -> String {
        match self {
            MemoryUnit::Bytes => format!("{bytes} B"),
            MemoryUnit::Kilobytes => format!("{:.1} KB", bytes as f64 / 1_000.0),
            MemoryUnit::Megabytes => format!("{:.1} MB", bytes as f64 / 1_000_000.0),
            MemoryUnit::Gigabytes => format!("{:.1} GB", bytes as f64 / 1_000_000_000.0),
            MemoryUnit::Kibibytes => format!("{:.1} KiB", bytes as f64 / 1_024.0),
            MemoryUnit::Mebibytes => format!("{:.1} MiB", bytes as f64 / 1_048_576.0),
            MemoryUnit::Gibibytes => format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "peak-mem",
    about = "Lightweight memory usage monitor for any process",
    author,
    long_about = "Peak-mem monitors and reports the peak memory usage of any program during its execution.\n\n\
                  It tracks both resident set size (RSS) and virtual memory size (VSZ) with minimal overhead.",
    disable_version_flag = true
)]
pub struct Cli {
    #[arg(
        trailing_var_arg = true,
        value_name = "COMMAND",
        help = "Command to execute and monitor",
        required_unless_present_any = &["list_baselines", "delete_baseline", "short_version", "long_version"]
    )]
    pub command: Vec<String>,

    #[arg(
        short = 'j',
        long = "json",
        help = "Output in JSON format",
        conflicts_with_all = &["csv", "quiet"]
    )]
    pub json: bool,

    #[arg(
        short = 'c',
        long = "csv",
        help = "Output in CSV format",
        conflicts_with_all = &["json", "quiet"]
    )]
    pub csv: bool,

    #[arg(
        short = 'q',
        long = "quiet",
        help = "Only output peak RSS value",
        conflicts_with_all = &["json", "csv", "verbose"]
    )]
    pub quiet: bool,

    #[arg(
        short = 'v',
        long = "verbose",
        help = "Show detailed breakdown",
        conflicts_with = "quiet"
    )]
    pub verbose: bool,

    #[arg(
        short = 'w',
        long = "watch",
        help = "Show real-time memory usage",
        conflicts_with_all = &["json", "csv", "quiet"]
    )]
    pub watch: bool,

    #[arg(
        short = 't',
        long = "threshold",
        value_name = "SIZE",
        help = "Set memory threshold (e.g., 512M, 1G)",
        value_parser = parse_threshold
    )]
    pub threshold: Option<ByteSize>,

    #[arg(
        long = "no-children",
        help = "Don't track child processes",
        action = ArgAction::SetTrue
    )]
    pub no_children: bool,

    #[arg(
        long = "timeline",
        value_name = "FILE",
        help = "Record memory timeline to file"
    )]
    pub timeline: Option<PathBuf>,

    #[arg(
        long = "interval",
        value_name = "MS",
        default_value = "100",
        help = "Sampling interval in milliseconds",
        value_parser = parse_interval
    )]
    pub interval: u64,

    #[arg(
        long = "units",
        value_name = "UNIT",
        help = "Force specific memory units (B, KB, MB, GB, KiB, MiB, GiB)",
        value_parser = parse_units
    )]
    pub units: Option<MemoryUnit>,

    #[arg(
        long = "save-baseline",
        value_name = "NAME",
        help = "Save the result as a baseline with the given name",
        conflicts_with = "compare_baseline"
    )]
    pub save_baseline: Option<String>,

    #[arg(
        long = "compare-baseline",
        value_name = "NAME",
        help = "Compare results against a saved baseline",
        conflicts_with = "save_baseline"
    )]
    pub compare_baseline: Option<String>,

    #[arg(
        long = "regression-threshold",
        value_name = "PERCENT",
        default_value = "10.0",
        help = "Memory increase percentage to consider as regression"
    )]
    pub regression_threshold: f64,

    #[arg(
        long = "baseline-dir",
        value_name = "DIR",
        help = "Directory to store baselines (default: ~/.cache/peak-mem/baselines)"
    )]
    pub baseline_dir: Option<PathBuf>,

    #[arg(
        long = "list-baselines",
        help = "List all saved baselines and exit",
        conflicts_with_all = &["command", "save_baseline", "compare_baseline"]
    )]
    pub list_baselines: bool,

    #[arg(
        long = "delete-baseline",
        value_name = "NAME",
        help = "Delete a saved baseline and exit",
        conflicts_with_all = &["command", "save_baseline", "compare_baseline", "list_baselines"]
    )]
    pub delete_baseline: Option<String>,

    #[arg(short = 'V', help = "Short version")]
    pub short_version: bool,

    #[arg(long = "version", help = "Long version info")]
    pub long_version: bool,
}

fn parse_threshold(s: &str) -> Result<ByteSize> {
    s.parse::<ByteSize>()
        .map_err(|_| anyhow::anyhow!("Invalid threshold format. Use formats like: 512M, 1G, 1.5GB"))
}

fn parse_interval(s: &str) -> Result<u64> {
    let interval: u64 = s.parse()?;
    if interval == 0 {
        anyhow::bail!("Interval must be greater than zero");
    }
    Ok(interval)
}

fn parse_units(s: &str) -> Result<MemoryUnit> {
    match s {
        "B" => Ok(MemoryUnit::Bytes),
        "KB" => Ok(MemoryUnit::Kilobytes),
        "MB" => Ok(MemoryUnit::Megabytes),
        "GB" => Ok(MemoryUnit::Gigabytes),
        "KiB" => Ok(MemoryUnit::Kibibytes),
        "MiB" => Ok(MemoryUnit::Mebibytes),
        "GiB" => Ok(MemoryUnit::Gibibytes),
        _ => anyhow::bail!("Invalid unit. Use one of: B, KB, MB, GB, KiB, MiB, GiB"),
    }
}

impl Cli {
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else if self.csv {
            OutputFormat::Csv
        } else if self.quiet {
            OutputFormat::Quiet
        } else {
            OutputFormat::Human
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
    Csv,
    Quiet,
}
