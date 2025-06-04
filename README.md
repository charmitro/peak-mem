# peak-mem

A lightweight, cross-platform memory usage monitor for any process.

## Overview

peak-mem is a command-line utility that monitors and reports the peak memory usage of any program during its execution. It tracks both resident set size (RSS) and virtual memory size (VSZ) with minimal overhead, providing system administrators and developers with essential memory consumption metrics.

## Features

- Low overhead monitoring
- Multiple output formats (human-readable, JSON, CSV, quiet)
- Child process tracking with process tree aggregation
- Real-time memory usage display
- Memory threshold monitoring with configurable alerts
- Timeline recording for memory usage analysis
- Cross-platform support (Linux, macOS, Windows & FreeBSD planned)
- Zero configuration required

## Installation

### From Source

```bash
cargo install --path .
```

### Pre-built Binaries

Download the latest release for your platform from the [releases page](https://github.com/peak-mem/peak-mem/releases).

## Usage

### Basic Usage

Monitor a command and report its peak memory usage:

```bash
peak-mem -- cargo build --release
```

Example output:
```
Command: cargo build --release
Peak memory usage: 487.3 MB (RSS) / 892.1 MB (VSZ)
Exit code: 0
Duration: 14.2s
```

### Command-line Options

```
USAGE:
    peak-mem [OPTIONS] -- <COMMAND> [ARGS...]

OPTIONS:
    -h, --help              Print help information
    -V, --version           Print version information
    -j, --json              Output in JSON format
    -c, --csv               Output in CSV format
    -q, --quiet             Only output peak RSS value in bytes
    -v, --verbose           Show detailed process breakdown
    -w, --watch             Display real-time memory usage
    -t, --threshold <SIZE>  Set memory threshold (e.g., 512M, 1G)
    --no-children           Don't track child processes
    --timeline <FILE>       Record memory timeline to file
    --interval <MS>         Sampling interval in milliseconds [default: 100]

ARGS:
    <COMMAND>               Command to execute and monitor
    [ARGS...]               Arguments to pass to the command
```

## Examples

### JSON Output

For integration with CI/CD pipelines or automated tools:

```bash
peak-mem --json -- ./my-app
```

Output:
```json
{
  "command": "./my-app",
  "peak_rss_bytes": 104857600,
  "peak_vsz_bytes": 209715200,
  "duration_ms": 5234,
  "exit_code": 0,
  "threshold_exceeded": false,
  "timestamp": "2025-05-24T10:30:45Z"
}
```

### Memory Threshold Monitoring

Exit with error code if memory exceeds threshold:

```bash
peak-mem --threshold 1GB -- ./memory-intensive-app
```

If the process exceeds 1GB of RSS, peak-mem will exit with code 1.

### Real-time Monitoring

Display live memory usage during execution:

```bash
peak-mem --watch -- ./long-running-process
```

### Exclude Child Processes

Monitor only the main process:

```bash
peak-mem --no-children -- make -j8
```

### Timeline Recording

Save memory usage over time for analysis:

```bash
peak-mem --timeline memory.json -- ./batch-job
```

### CSV Output

For spreadsheet import or data analysis:

```bash
peak-mem --csv -- ./data-processor
```

Output:
```csv
command,peak_rss_bytes,peak_vsz_bytes,duration_ms,exit_code,threshold_exceeded,timestamp
./data-processor,52428800,104857600,2150,0,false,2025-05-24T10:30:45+00:00
```

## Platform Support

| Platform | Status | Implementation |
|----------|--------|---------------|
| Linux    | Stable | `/proc` filesystem |
| macOS    | Stable | `proc_pidinfo` APIs |
| Windows  | Planned | Windows API |
| FreeBSD  | Planned | `sysctl` interface |

## How It Works

peak-mem operates by:

1. Spawning the target process as a child
2. Periodically sampling memory statistics (default: every 100ms)
3. Tracking all descendant processes if enabled
4. Recording peak values throughout execution
5. Reporting results when the process terminates

The monitoring is performed using platform-specific APIs to minimize overhead:
- **Linux**: Reads from `/proc/[pid]/status` and `/proc/[pid]/stat`
- **macOS**: Uses `proc_pidinfo` system calls
- **Windows**: Will use `GetProcessMemoryInfo` (planned)
- **FreeBSD**: Will use `sysctl` and `kvm` interfaces (planned)

## Performance Characteristics

- Startup time: < 10ms
- Memory overhead: < 10MB
- CPU overhead: < 1%
- Binary size: ~1.1MB (release build, stripped)
- Sampling rate: Configurable, default 10Hz

## Building from Source

### Prerequisites

- Rust

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with verbose output
RUST_LOG=debug cargo run -- <command>
```

### Cross-compilation

```bash
# For Linux x86_64
cargo build --target x86_64-unknown-linux-gnu

# For macOS x86_64
cargo build --target x86_64-apple-darwin

# For macOS ARM64
cargo build --target aarch64-apple-darwin
```

## Development

### Project Structure

```
src/
├── main.rs          # Application entry point
├── cli.rs           # Command-line argument parsing
├── types.rs         # Core data structures
├── process/         # Process spawning and management
├── monitor/         # Platform-specific memory monitoring
│   ├── mod.rs       # Platform abstraction layer
│   ├── linux.rs     # Linux implementation
│   ├── macos.rs     # macOS implementation
│   ├── windows.rs   # Windows implementation (stub)
│   └── tracker.rs   # Memory tracking logic
└── output/          # Output formatting
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_memory_tracking
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Security audit
cargo audit

# Check dependencies
cargo deny check
```

## Configuration

peak-mem requires no configuration files. All options are specified via command-line arguments.

Environment variables:
- `RUST_LOG`: Set to `debug` for verbose logging

## Troubleshooting

### Permission Denied

On some systems, you may need elevated permissions to monitor certain processes:
- Linux: No special permissions required for own processes
- macOS: May require developer tools or sudo for system processes

### High Memory Usage Reported

Virtual memory size (VSZ) includes all mapped memory and is typically much larger than RSS. Focus on RSS for actual physical memory usage.

### Child Processes Not Tracked

Some programs may spawn processes in ways that break the parent-child relationship. Use system-specific tools like `pstree` to verify process relationships.

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and clippy
5. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

This project was inspired by:
- GNU time's memory reporting capabilities
- The need for a simple, cross-platform memory monitoring solution
- The Rust ecosystem's excellent system programming capabilities

## Related Projects

- [GNU time](https://www.gnu.org/software/time/) - Classic UNIX time with memory reporting
- [hyperfine](https://github.com/sharkdp/hyperfine) - Command-line benchmarking tool
- [procs](https://github.com/dalance/procs) - Modern ps replacement written in Rust
