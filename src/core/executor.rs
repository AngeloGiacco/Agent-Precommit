//! Command execution for running checks.
//!
//! This module provides utilities for executing shell commands
//! with timeout support, output capture, and error handling.

use crate::core::error::{Error, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

/// Output from a command execution.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Exit code of the command.
    pub exit_code: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Whether the command was killed due to timeout.
    pub timed_out: bool,
    /// Duration the command took to run.
    pub duration: Duration,
}

impl CommandOutput {
    /// Returns true if the command succeeded (exit code 0).
    #[must_use]
    pub const fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }

    /// Returns combined stdout and stderr output.
    #[must_use]
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// Options for command execution.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// Working directory for the command.
    pub cwd: Option<std::path::PathBuf>,
    /// Timeout for the command.
    pub timeout: Option<Duration>,
    /// Environment variables to set.
    pub env: Vec<(String, String)>,
    /// Whether to capture output (vs streaming to console).
    pub capture_output: bool,
    /// Shell to use (default: sh on Unix, cmd on Windows).
    pub shell: Option<String>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            timeout: Some(Duration::from_secs(300)), // 5 minutes default
            env: Vec::new(),
            capture_output: true,
            shell: None,
        }
    }
}

impl ExecuteOptions {
    /// Sets the working directory.
    #[must_use]
    pub fn cwd(mut self, path: impl AsRef<Path>) -> Self {
        self.cwd = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the timeout.
    #[must_use]
    pub const fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    /// Sets an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Sets whether to capture output.
    #[must_use]
    pub const fn capture_output(mut self, capture: bool) -> Self {
        self.capture_output = capture;
        self
    }
}

/// Executor for running shell commands.
#[derive(Debug, Default)]
pub struct Executor;

impl Executor {
    /// Creates a new executor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Executes a shell command.
    pub async fn execute(&self, command: &str, options: ExecuteOptions) -> Result<CommandOutput> {
        let start = std::time::Instant::now();

        // Determine shell
        let (shell, shell_arg) = if cfg!(windows) {
            (
                options.shell.as_deref().unwrap_or("cmd"),
                "/C",
            )
        } else {
            (
                options.shell.as_deref().unwrap_or("sh"),
                "-c",
            )
        };

        // Build command
        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg).arg(command);

        // Set working directory
        if let Some(ref cwd) = options.cwd {
            cmd.current_dir(cwd);
        }

        // Set environment variables
        for (key, value) in &options.env {
            cmd.env(key, value);
        }

        // Configure output handling
        cmd.stdin(Stdio::null());

        if options.capture_output {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        } else {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        }

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| Error::io("spawn command", e))?;

        // Handle timeout
        let result = if let Some(timeout_duration) = options.timeout {
            match timeout(timeout_duration, async {
                self.wait_for_output(&mut child, options.capture_output).await
            })
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    // Kill the process on timeout - ignore result since we're returning anyway
                    drop(child.kill().await);
                    return Ok(CommandOutput {
                        exit_code: 124,
                        stdout: String::new(),
                        stderr: "Command timed out".to_string(),
                        timed_out: true,
                        duration: start.elapsed(),
                    });
                }
            }
        } else {
            self.wait_for_output(&mut child, options.capture_output)
                .await
        };

        let (exit_code, stdout, stderr) = result?;

        Ok(CommandOutput {
            exit_code,
            stdout,
            stderr,
            timed_out: false,
            duration: start.elapsed(),
        })
    }

    /// Waits for the command to complete and captures output.
    async fn wait_for_output(
        &self,
        child: &mut tokio::process::Child,
        capture: bool,
    ) -> Result<(i32, String, String)> {
        if capture {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let stdout_handle = tokio::spawn(async move {
                let mut output = String::new();
                if let Some(stdout) = stdout {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        output.push_str(&line);
                        output.push('\n');
                    }
                }
                output
            });

            let stderr_handle = tokio::spawn(async move {
                let mut output = String::new();
                if let Some(stderr) = stderr {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        output.push_str(&line);
                        output.push('\n');
                    }
                }
                output
            });

            let status = child.wait().await.map_err(|e| Error::io("wait for command", e))?;

            let stdout = stdout_handle
                .await
                .map_err(|e| Error::Internal {
                    message: format!("stdout task failed: {e}"),
                })?;
            let stderr = stderr_handle
                .await
                .map_err(|e| Error::Internal {
                    message: format!("stderr task failed: {e}"),
                })?;

            Ok((status.code().unwrap_or(1), stdout, stderr))
        } else {
            let status = child.wait().await.map_err(|e| Error::io("wait for command", e))?;
            Ok((status.code().unwrap_or(1), String::new(), String::new()))
        }
    }

    /// Checks if a command exists in PATH.
    #[must_use]
    pub fn command_exists(command: &str) -> bool {
        which::which(command).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_simple_command() {
        let executor = Executor::new();
        let result = executor
            .execute("echo hello", ExecuteOptions::default())
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.success());
        assert!(output.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        let executor = Executor::new();
        let result = executor
            .execute("exit 1", ExecuteOptions::default())
            .await;

        assert!(result.is_ok());
        let output = result.expect("should complete");
        assert!(!output.success());
        assert_eq!(output.exit_code, 1);
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let executor = Executor::new();
        let result = executor
            .execute(
                "sleep 10",
                ExecuteOptions::default().timeout(Duration::from_millis(100)),
            )
            .await;

        assert!(result.is_ok());
        let output = result.expect("should complete");
        assert!(output.timed_out);
        assert_eq!(output.exit_code, 124);
    }

    #[test]
    fn test_command_exists() {
        // 'sh' should exist on Unix, 'cmd' on Windows
        if cfg!(unix) {
            assert!(Executor::command_exists("sh"));
        } else {
            assert!(Executor::command_exists("cmd"));
        }

        // This should not exist
        assert!(!Executor::command_exists("definitely_not_a_real_command_12345"));
    }
}
