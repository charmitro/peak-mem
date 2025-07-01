//! Process spawning and management.
//!
//! This module handles spawning the target process and managing its lifecycle,
//! including signal forwarding on Unix systems.

use crate::types::{PeakMemError, Result};
use std::process::Stdio;
use tokio::process::Command;

/// Handles spawning and running the target process.
pub struct ProcessRunner {
    command: Vec<String>,
}

impl ProcessRunner {
    /// Creates a new process runner with the given command.
    ///
    /// # Arguments
    /// * `command` - Command and arguments to execute
    ///
    /// # Errors
    /// * Returns error if command is empty
    pub fn new(command: Vec<String>) -> Result<Self> {
        if command.is_empty() {
            return Err(PeakMemError::ProcessSpawn(
                "No command provided".to_string(),
            ));
        }

        Ok(Self { command })
    }

    /// Spawns the configured process.
    ///
    /// The process inherits stdin, stdout, and stderr from the parent.
    ///
    /// # Returns
    /// * `ProcessHandle` for managing the spawned process
    pub async fn spawn(&self) -> Result<ProcessHandle> {
        let program = &self.command[0];
        let args = &self.command[1..];

        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let child = cmd
            .spawn()
            .map_err(|e| PeakMemError::ProcessSpawn(format!("Failed to spawn '{program}': {e}")))?;

        let pid = child
            .id()
            .ok_or_else(|| PeakMemError::ProcessSpawn("Failed to get process ID".to_string()))?;

        Ok(ProcessHandle { child, pid })
    }

    /// Returns the command as a single string for display.
    pub fn command_string(&self) -> String {
        self.command.join(" ")
    }
}

/// Handle to a spawned process.
///
/// Provides methods for waiting on the process and forwarding signals.
pub struct ProcessHandle {
    child: tokio::process::Child,
    pid: u32,
}

impl ProcessHandle {
    /// Returns the process ID.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Waits for the process to complete while forwarding signals on Unix.
    ///
    /// Forwards SIGINT and SIGTERM to the child process.
    ///
    /// # Returns
    /// * Exit code of the process
    #[cfg(unix)]
    pub async fn wait_with_signal_forwarding(mut self) -> Result<Option<i32>> {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        use tokio::signal::unix::{signal, SignalKind};

        let child_pid = Pid::from_raw(self.pid as i32);

        // Set up signal handlers
        let mut sigint_stream = signal(SignalKind::interrupt())?;
        let mut sigterm_stream = signal(SignalKind::terminate())?;

        // Wait for either the child to exit or a signal
        tokio::select! {
            // Child process exited
            status = self.child.wait() => {
                Ok(status?.code())
            }
            // SIGINT received (Ctrl+C)
            _ = sigint_stream.recv() => {
                // Forward SIGINT to child
                let _ = signal::kill(child_pid, Signal::SIGINT);
                // Wait for child to exit
                let status = self.child.wait().await?;
                Ok(status.code())
            }
            // SIGTERM received
            _ = sigterm_stream.recv() => {
                // Forward SIGTERM to child
                let _ = signal::kill(child_pid, Signal::SIGTERM);
                // Wait for child to exit
                let status = self.child.wait().await?;
                Ok(status.code())
            }
        }
    }

    /// Waits for the process to complete on Windows.
    ///
    /// On Windows, Ctrl+C is automatically forwarded to child processes
    /// in the same console.
    ///
    /// # Returns
    /// * Exit code of the process
    #[cfg(windows)]
    pub async fn wait_with_signal_forwarding(mut self) -> Result<Option<i32>> {
        // On Windows, Ctrl+C is automatically forwarded to child processes
        // in the same console, so we just wait normally
        let status = self.child.wait().await?;
        Ok(status.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_runner() {
        let runner = ProcessRunner::new(vec!["echo".to_string(), "test".to_string()]).unwrap();
        let handle = runner.spawn().await.unwrap();
        let pid = handle.pid();
        assert!(pid > 0);

        let exit_code = handle.wait_with_signal_forwarding().await.unwrap();
        assert_eq!(exit_code, Some(0));
    }

    #[test]
    fn test_empty_command() {
        let result = ProcessRunner::new(vec![]);
        assert!(result.is_err());
    }
}
