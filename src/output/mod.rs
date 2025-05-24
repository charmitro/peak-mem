use crate::cli::OutputFormat;
use crate::types::MonitorResult;
use anyhow::Result;
use bytesize::ByteSize;
use std::io::{self, Write};

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn format(result: &MonitorResult, format: OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Human => Self::format_human(result),
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
        };

        // Quiet format should just print the RSS bytes
        OutputFormatter::format(&result, OutputFormat::Quiet).unwrap();
    }
}
