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
            (options.shell.as_deref().unwrap_or("cmd"), "/C")
        } else {
            (options.shell.as_deref().unwrap_or("sh"), "-c")
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
                self.wait_for_output(&mut child, options.capture_output)
                    .await
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
                },
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

            let status = child
                .wait()
                .await
                .map_err(|e| Error::io("wait for command", e))?;

            let stdout = stdout_handle.await.map_err(|e| Error::Internal {
                message: format!("stdout task failed: {e}"),
            })?;
            let stderr = stderr_handle.await.map_err(|e| Error::Internal {
                message: format!("stderr task failed: {e}"),
            })?;

            Ok((status.code().unwrap_or(1), stdout, stderr))
        } else {
            let status = child
                .wait()
                .await
                .map_err(|e| Error::io("wait for command", e))?;
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

    // =========================================================================
    // CommandOutput tests
    // =========================================================================

    #[test]
    fn test_command_output_success() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: "test".to_string(),
            stderr: String::new(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        assert!(output.success());
    }

    #[test]
    fn test_command_output_failure_exit_code() {
        let output = CommandOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error".to_string(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        assert!(!output.success());
    }

    #[test]
    fn test_command_output_failure_timeout() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            duration: Duration::from_secs(1),
        };
        assert!(!output.success());
    }

    #[test]
    fn test_command_output_combined_output_stdout_only() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: "stdout content".to_string(),
            stderr: String::new(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        assert_eq!(output.combined_output(), "stdout content");
    }

    #[test]
    fn test_command_output_combined_output_stderr_only() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: "stderr content".to_string(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        assert_eq!(output.combined_output(), "stderr content");
    }

    #[test]
    fn test_command_output_combined_output_both() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: "stdout".to_string(),
            stderr: "stderr".to_string(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        let combined = output.combined_output();
        assert!(combined.contains("stdout"));
        assert!(combined.contains("stderr"));
    }

    #[test]
    fn test_command_output_combined_output_empty() {
        let output = CommandOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            duration: Duration::from_secs(1),
        };
        assert!(output.combined_output().is_empty());
    }

    // =========================================================================
    // ExecuteOptions tests
    // =========================================================================

    #[test]
    fn test_execute_options_default() {
        let options = ExecuteOptions::default();
        assert!(options.cwd.is_none());
        assert!(options.timeout.is_some());
        assert_eq!(options.timeout, Some(Duration::from_secs(300)));
        assert!(options.env.is_empty());
        assert!(options.capture_output);
        assert!(options.shell.is_none());
    }

    #[test]
    fn test_execute_options_cwd() {
        let options = ExecuteOptions::default().cwd("/tmp");
        assert_eq!(options.cwd, Some(std::path::PathBuf::from("/tmp")));
    }

    #[test]
    fn test_execute_options_timeout() {
        let options = ExecuteOptions::default().timeout(Duration::from_secs(60));
        assert_eq!(options.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_execute_options_env() {
        let options = ExecuteOptions::default()
            .env("KEY1", "VALUE1")
            .env("KEY2", "VALUE2");
        assert_eq!(options.env.len(), 2);
        assert!(options
            .env
            .contains(&("KEY1".to_string(), "VALUE1".to_string())));
        assert!(options
            .env
            .contains(&("KEY2".to_string(), "VALUE2".to_string())));
    }

    #[test]
    fn test_execute_options_capture_output() {
        let options = ExecuteOptions::default().capture_output(false);
        assert!(!options.capture_output);
    }

    #[test]
    fn test_execute_options_chaining() {
        let options = ExecuteOptions::default()
            .cwd("/tmp")
            .timeout(Duration::from_secs(60))
            .env("TEST", "value")
            .capture_output(false);

        assert_eq!(options.cwd, Some(std::path::PathBuf::from("/tmp")));
        assert_eq!(options.timeout, Some(Duration::from_secs(60)));
        assert_eq!(options.env.len(), 1);
        assert!(!options.capture_output);
    }

    // =========================================================================
    // Executor tests
    // =========================================================================

    #[test]
    fn test_executor_new() {
        let executor = Executor::new();
        // Verify we can create an executor
        let debug_str = format!("{:?}", executor);
        assert!(debug_str.contains("Executor"));
    }

    #[test]
    fn test_executor_default() {
        let executor = Executor::default();
        let debug_str = format!("{:?}", executor);
        assert!(debug_str.contains("Executor"));
    }

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
        let result = executor.execute("exit 1", ExecuteOptions::default()).await;

        assert!(result.is_ok());
        let output = result.expect("should complete");
        assert!(!output.success());
        assert_eq!(output.exit_code, 1);
    }

    #[tokio::test]
    async fn test_execute_various_exit_codes() {
        let executor = Executor::new();

        for exit_code in [0, 1, 2, 42, 127] {
            let result = executor
                .execute(&format!("exit {}", exit_code), ExecuteOptions::default())
                .await;

            assert!(result.is_ok());
            let output = result.expect("should complete");
            assert_eq!(output.exit_code, exit_code);
        }
    }

    #[tokio::test]
    async fn test_execute_with_stderr() {
        let executor = Executor::new();
        let result = executor
            .execute("echo error >&2", ExecuteOptions::default())
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.success());
        assert!(output.stderr.contains("error"));
    }

    #[tokio::test]
    async fn test_execute_with_environment_variable() {
        let executor = Executor::new();
        let result = executor
            .execute(
                "echo $TEST_VAR",
                ExecuteOptions::default().env("TEST_VAR", "test_value"),
            )
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.success());
        assert!(output.stdout.contains("test_value"));
    }

    #[tokio::test]
    async fn test_execute_with_working_directory() {
        let executor = Executor::new();
        let result = executor
            .execute("pwd", ExecuteOptions::default().cwd("/tmp"))
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.success());
        assert!(output.stdout.contains("/tmp") || output.stdout.contains("tmp"));
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

    #[tokio::test]
    async fn test_execute_duration_is_recorded() {
        let executor = Executor::new();
        let result = executor
            .execute("sleep 0.1", ExecuteOptions::default())
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.duration >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_execute_multiline_output() {
        let executor = Executor::new();
        let result = executor
            .execute(
                "echo line1; echo line2; echo line3",
                ExecuteOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.success());
        assert!(output.stdout.contains("line1"));
        assert!(output.stdout.contains("line2"));
        assert!(output.stdout.contains("line3"));
    }

    #[test]
    fn test_command_exists() {
        // 'sh' should exist on Unix, 'cmd' on Windows
        if cfg!(unix) {
            assert!(Executor::command_exists("sh"));
            assert!(Executor::command_exists("echo"));
        } else {
            assert!(Executor::command_exists("cmd"));
        }

        // This should not exist
        assert!(!Executor::command_exists(
            "definitely_not_a_real_command_12345"
        ));
    }

    #[test]
    fn test_command_exists_common_tools() {
        // These should exist on most systems
        if cfg!(unix) {
            assert!(Executor::command_exists("ls"));
            assert!(Executor::command_exists("cat"));
        }
    }
}
