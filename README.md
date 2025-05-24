# peak-mem

[![CI](https://github.com/peak-mem/peak-mem/actions/workflows/ci.yml/badge.svg)](https://github.com/peak-mem/peak-mem/actions/workflows/ci.yml)
[![Security Audit](https://github.com/peak-mem/peak-mem/actions/workflows/security.yml/badge.svg)](https://github.com/peak-mem/peak-mem/actions/workflows/security.yml)

A lightweight, cross-platform memory usage monitor for any process. Track peak memory consumption with minimal overhead.

## Features

- 🚀 **Minimal overhead** - Less than 1% CPU impact on monitored processes
- 📊 **Multiple output formats** - Human-readable, JSON, CSV, or quiet mode
- 👶 **Child process tracking** - Monitor entire process trees
- ⚡ **Real-time monitoring** - Watch memory usage as it happens
- 🎯 **Threshold alerts** - Exit with error code when memory exceeds limits
- 📈 **Timeline recording** - Export memory usage over time
- 🔧 **Zero configuration** - Works immediately after installation

## Installation

### From source

```bash
cargo install --path .
```

### Pre-built binaries

Download the latest release for your platform from the [releases page](https://github.com/peak-mem/peak-mem/releases).

## Usage

### Basic usage

Monitor a command and report its peak memory usage:

```bash
peak-mem -- cargo build --release
```

Output:
```
Command: cargo build --release
Peak memory usage: 487.3 MB (RSS) / 892.1 MB (VSZ)
Exit code: 0
Duration: 14.2s
```

### Command-line options

```
peak-mem [OPTIONS] -- <COMMAND> [ARGS...]

OPTIONS:
    -h, --help              Print help information
    -V, --version           Print version information
    -j, --json              Output in JSON format
    -c, --csv               Output in CSV format
    -q, --quiet             Only output peak RSS value
    -v, --verbose           Show detailed breakdown
    -w, --watch             Show real-time memory usage
    -t, --threshold <SIZE>  Set memory threshold (e.g., 512M, 1G)
    --no-children           Don't track child processes
    --timeline <FILE>       Record memory timeline to file
    --interval <MS>         Sampling interval in milliseconds [default: 100]
```

### Examples

#### JSON output for CI/CD integration

```bash
peak-mem --json -- ./my-app
```

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

#### Set memory threshold

```bash
peak-mem --threshold 1GB -- ./memory-intensive-app
```

If the process exceeds 1GB of RSS, peak-mem will exit with code 1.

#### Real-time monitoring

```bash
peak-mem --watch -- ./long-running-process
```

Shows live memory usage updates during execution.

#### Monitor without child processes

```bash
peak-mem --no-children -- make -j8
```

Only tracks the main make process, not the spawned compilation jobs.

#### Record memory timeline

```bash
peak-mem --timeline memory.json -- ./batch-job
```

Saves detailed memory usage over time for later analysis.

## Platform Support

- ✅ **Linux** - Full support via `/proc` filesystem
- ✅ **macOS** - Full support via system APIs
- 🚧 **Windows** - Planned
- 🚧 **FreeBSD** - Planned

## How it works

peak-mem spawns your process and monitors its memory usage by:

1. Sampling memory statistics at regular intervals (default: 100ms)
2. Tracking all child processes in the process tree
3. Recording peak values throughout execution
4. Reporting results after the process terminates

The tool uses platform-specific APIs for minimal overhead:
- Linux: `/proc/[pid]/status` for memory information
- macOS: `proc_pidinfo` system calls
- Windows: `GetProcessMemoryInfo` (planned)

## Performance

- **Startup time**: < 10ms
- **Memory overhead**: < 10MB
- **CPU overhead**: < 1%
- **Binary size**: ~1.1MB (stripped)

## Building from source

### Prerequisites

- Rust 1.70 or later
- Cargo

### Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### Run tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development

The codebase is organized as follows:

- `src/cli.rs` - Command-line argument parsing
- `src/monitor/` - Platform-specific memory monitoring
- `src/process/` - Process spawning and management
- `src/output/` - Output formatting
- `src/types.rs` - Core data types

## License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

Inspired by GNU time's memory reporting and the need for a simple, cross-platform memory monitoring tool.