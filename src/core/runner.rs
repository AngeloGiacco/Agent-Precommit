//! Check runner for executing pre-commit checks.
//!
//! This module orchestrates the execution of checks based on the detected mode.

// Allow this for Rust 2024 compatibility - the drop order change is harmless here
#![allow(tail_expr_drop_order)]

use crate::config::{CheckConfig, Config};
use crate::core::detector::Mode;
use crate::core::error::{Error, Result};
use crate::core::executor::{CommandOutput, ExecuteOptions, Executor};
use crate::core::git::GitRepo;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Result of running a single check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Name of the check.
    pub name: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Output from the check.
    pub output: CommandOutput,
    /// Whether the check was skipped.
    pub skipped: bool,
    /// Reason for skipping (if skipped).
    pub skip_reason: Option<String>,
}

impl CheckResult {
    /// Creates a skipped check result.
    fn skipped(name: String, reason: String) -> Self {
        Self {
            name,
            passed: true, // Skipped checks don't fail
            output: CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                duration: Duration::ZERO,
            },
            skipped: true,
            skip_reason: Some(reason),
        }
    }
}

/// Result of running all checks.
#[derive(Debug)]
pub struct RunResult {
    /// Mode that was used.
    pub mode: Mode,
    /// Individual check results.
    pub checks: Vec<CheckResult>,
    /// Total duration.
    pub duration: Duration,
}

impl RunResult {
    /// Returns true if all checks passed.
    #[must_use]
    pub fn success(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    /// Returns the number of passed checks.
    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.passed && !c.skipped)
            .count()
    }

    /// Returns the number of failed checks.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.checks.iter().filter(|c| !c.passed).count()
    }

    /// Returns the number of skipped checks.
    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.checks.iter().filter(|c| c.skipped).count()
    }

    /// Returns failed check results.
    #[must_use]
    pub fn failed_checks(&self) -> impl Iterator<Item = &CheckResult> {
        self.checks.iter().filter(|c| !c.passed)
    }
}

/// Runner for executing checks.
#[derive(Debug)]
pub struct Runner {
    config: Config,
    repo: Option<GitRepo>,
}

impl Runner {
    /// Creates a new runner with the given configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            repo: GitRepo::discover().ok(),
        }
    }

    /// Creates a new runner with a specific repository.
    #[must_use]
    pub fn with_repo(config: Config, repo: GitRepo) -> Self {
        Self {
            config,
            repo: Some(repo),
        }
    }

    /// Runs checks for the given mode.
    pub async fn run(&self, mode: Mode) -> Result<RunResult> {
        let start = std::time::Instant::now();

        // Get checks for this mode
        let check_names = self.get_checks_for_mode(mode);

        if check_names.is_empty() {
            return Ok(RunResult {
                mode,
                checks: Vec::new(),
                duration: start.elapsed(),
            });
        }

        // Resolve check configurations
        let checks = self.resolve_checks(&check_names)?;

        // Run checks based on mode settings
        let results = if mode.is_thorough() {
            self.run_parallel_groups(mode, &checks).await?
        } else {
            self.run_sequential(mode, &checks).await?
        };

        Ok(RunResult {
            mode,
            checks: results,
            duration: start.elapsed(),
        })
    }

    /// Runs a single check by name.
    pub async fn run_single(&self, name: &str, mode: Mode) -> Result<CheckResult> {
        let check = self
            .config
            .checks
            .get(name)
            .ok_or_else(|| Error::CheckNotFound {
                name: name.to_string(),
            })?;

        self.run_check(name, check, mode).await
    }

    /// Gets the list of checks for a mode.
    fn get_checks_for_mode(&self, mode: Mode) -> Vec<String> {
        match mode {
            Mode::Human => self.config.human.checks.clone(),
            Mode::Agent | Mode::Ci => self.config.agent.checks.clone(),
        }
    }

    /// Resolves check names to configurations.
    fn resolve_checks(&self, names: &[String]) -> Result<Vec<(String, CheckConfig)>> {
        let mut checks = Vec::with_capacity(names.len());

        for name in names {
            let check = self
                .config
                .checks
                .get(name)
                .cloned()
                .unwrap_or_else(|| CheckConfig::from_command(name.clone()));

            checks.push((name.clone(), check));
        }

        Ok(checks)
    }

    /// Runs checks sequentially (for human mode).
    async fn run_sequential(
        &self,
        mode: Mode,
        checks: &[(String, CheckConfig)],
    ) -> Result<Vec<CheckResult>> {
        let mut results = Vec::with_capacity(checks.len());

        for (name, check) in checks {
            let result = self.run_check(name, check, mode).await?;

            let failed = !result.passed;
            results.push(result);

            // Fail fast in human mode
            if failed && self.config.human.fail_fast {
                break;
            }
        }

        Ok(results)
    }

    /// Runs checks in parallel groups (for agent mode).
    async fn run_parallel_groups(
        &self,
        mode: Mode,
        checks: &[(String, CheckConfig)],
    ) -> Result<Vec<CheckResult>> {
        let check_map: HashMap<_, _> = checks.iter().cloned().collect();

        // Get parallel groups or create default groups
        let groups = if self.config.agent.parallel_groups.is_empty() {
            // Default: run all checks in parallel
            vec![checks.iter().map(|(n, _)| n.clone()).collect()]
        } else {
            self.config.agent.parallel_groups.clone()
        };

        let mut all_results = Vec::new();
        let semaphore = Arc::new(Semaphore::new(concurrency::available_parallelism()));

        for group in groups {
            let group_checks: Vec<_> = group
                .iter()
                .filter_map(|name| check_map.get(name).map(|c| (name.clone(), c.clone())))
                .collect();

            if group_checks.is_empty() {
                continue;
            }

            let mut handles = Vec::new();

            for (name, check) in group_checks {
                let sem = Arc::clone(&semaphore);
                let config = self.config.clone();
                let repo = self.repo.clone();

                handles.push(tokio::spawn(async move {
                    // Acquire semaphore permit; if semaphore is closed, treat as internal error
                    let _permit = sem.acquire().await.map_err(|_| Error::Internal {
                        message: "Semaphore closed unexpectedly".to_string(),
                    })?;
                    run_check_async(&name, &check, mode, &config, repo.as_ref()).await
                }));
            }

            for handle in handles {
                match handle.await {
                    Ok(result) => all_results.push(result?),
                    Err(e) => {
                        return Err(Error::Internal {
                            message: format!("Task join error: {e}"),
                        });
                    },
                }
            }

            // Check for failures if not running all checks
            if !self.config.agent.fail_fast {
                continue;
            }

            if all_results.iter().any(|r: &CheckResult| !r.passed) {
                break;
            }
        }

        Ok(all_results)
    }

    /// Runs a single check.
    async fn run_check(&self, name: &str, check: &CheckConfig, mode: Mode) -> Result<CheckResult> {
        run_check_async(name, check, mode, &self.config, self.repo.as_ref()).await
    }
}

/// Runs a check asynchronously (for parallel execution).
async fn run_check_async(
    name: &str,
    check: &CheckConfig,
    mode: Mode,
    config: &Config,
    repo: Option<&GitRepo>,
) -> Result<CheckResult> {
    // Check if the check is enabled
    if !check_enabled(check, repo) {
        return Ok(CheckResult::skipped(
            name.to_string(),
            "Condition not met".to_string(),
        ));
    }

    // Build execution options
    let timeout_str = match mode {
        Mode::Human => &config.human.timeout,
        Mode::Agent | Mode::Ci => &config.agent.timeout,
    };

    let timeout = parse_duration(timeout_str).unwrap_or_else(|| {
        tracing::warn!(
            timeout_str = %timeout_str,
            default_secs = 300,
            "Invalid timeout format, using default"
        );
        Duration::from_secs(300)
    });

    let mut options = ExecuteOptions::default().timeout(timeout);

    if let Some(repo) = repo {
        options = options.cwd(repo.root());
    }

    // Add environment variables from check config
    for (key, value) in &check.env {
        options = options.env(key.clone(), value.clone());
    }

    // Execute the command
    let executor = Executor::new();

    // Show progress
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .ok()
            .unwrap_or_else(ProgressStyle::default_spinner),
    );
    pb.set_message(format!("Running {name}..."));
    pb.enable_steady_tick(Duration::from_millis(100));

    let output = executor.execute(&check.run, options).await?;

    pb.finish_and_clear();

    // Format result
    if output.success() {
        eprintln!("{} {name}", style("✓").green());
    } else if output.timed_out {
        eprintln!("{} {name} (timed out)", style("✗").red());
    } else {
        eprintln!("{} {name}", style("✗").red());
    }

    Ok(CheckResult {
        name: name.to_string(),
        passed: output.success(),
        output,
        skipped: false,
        skip_reason: None,
    })
}

/// Checks if a check is enabled based on its conditions.
fn check_enabled(check: &CheckConfig, repo: Option<&GitRepo>) -> bool {
    let Some(ref condition) = check.enabled_if else {
        return true;
    };

    // Check file_exists condition
    if let Some(ref path) = condition.file_exists {
        if let Some(repo) = repo {
            if !repo.file_exists(path) {
                return false;
            }
        }
    }

    // Check dir_exists condition
    if let Some(ref path) = condition.dir_exists {
        if let Some(repo) = repo {
            if !repo.dir_exists(path) {
                return false;
            }
        }
    }

    // Check command_exists condition
    if let Some(ref cmd) = condition.command_exists {
        if !Executor::command_exists(cmd) {
            return false;
        }
    }

    true
}

/// Parses a duration string like "30s", "5m", "1h".
fn parse_duration(s: &str) -> Option<Duration> {
    humantime::parse_duration(s).ok()
}

/// Concurrency utilities for parallel execution.
mod concurrency {
    /// Returns the number of available CPU cores for parallel execution.
    pub fn available_parallelism() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Helper functions for tests
    // =========================================================================

    fn make_passed_check(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: true,
            output: CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                duration: Duration::ZERO,
            },
            skipped: false,
            skip_reason: None,
        }
    }

    fn make_failed_check(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: false,
            output: CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: "Error".to_string(),
                timed_out: false,
                duration: Duration::ZERO,
            },
            skipped: false,
            skip_reason: None,
        }
    }

    fn make_skipped_check(name: &str) -> CheckResult {
        CheckResult::skipped(name.to_string(), "Condition not met".to_string())
    }

    // =========================================================================
    // parse_duration tests
    // =========================================================================

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("1s"), Some(Duration::from_secs(1)));
        assert_eq!(parse_duration("0s"), Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1m"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("15m"), Some(Duration::from_secs(900)));
    }

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn test_parse_duration_complex() {
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("1m30s"), Some(Duration::from_secs(90)));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert_eq!(parse_duration("invalid"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc123"), None);
    }

    // =========================================================================
    // CheckResult tests
    // =========================================================================

    #[test]
    fn test_check_result_skipped() {
        let result = CheckResult::skipped("test".to_string(), "reason".to_string());
        assert!(result.passed);
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("reason".to_string()));
        assert_eq!(result.output.exit_code, 0);
        assert!(!result.output.timed_out);
    }

    #[test]
    fn test_check_result_debug() {
        let result = make_passed_check("test");
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("test"));
    }

    // =========================================================================
    // RunResult tests
    // =========================================================================

    #[test]
    fn test_run_result_success_all_passed() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![make_passed_check("test1"), make_passed_check("test2")],
            duration: Duration::ZERO,
        };

        assert!(result.success());
        assert_eq!(result.passed_count(), 2);
        assert_eq!(result.failed_count(), 0);
        assert_eq!(result.skipped_count(), 0);
    }

    #[test]
    fn test_run_result_failure_one_failed() {
        let result = RunResult {
            mode: Mode::Agent,
            checks: vec![make_passed_check("test1"), make_failed_check("test2")],
            duration: Duration::ZERO,
        };

        assert!(!result.success());
        assert_eq!(result.passed_count(), 1);
        assert_eq!(result.failed_count(), 1);
        assert_eq!(result.skipped_count(), 0);
    }

    #[test]
    fn test_run_result_all_failed() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![make_failed_check("test1"), make_failed_check("test2")],
            duration: Duration::ZERO,
        };

        assert!(!result.success());
        assert_eq!(result.passed_count(), 0);
        assert_eq!(result.failed_count(), 2);
    }

    #[test]
    fn test_run_result_with_skipped_checks() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![
                make_passed_check("test1"),
                make_skipped_check("test2"),
                make_passed_check("test3"),
            ],
            duration: Duration::ZERO,
        };

        assert!(result.success());
        assert_eq!(result.passed_count(), 2);
        assert_eq!(result.failed_count(), 0);
        assert_eq!(result.skipped_count(), 1);
    }

    #[test]
    fn test_run_result_all_skipped() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![make_skipped_check("test1"), make_skipped_check("test2")],
            duration: Duration::ZERO,
        };

        assert!(result.success());
        assert_eq!(result.passed_count(), 0);
        assert_eq!(result.failed_count(), 0);
        assert_eq!(result.skipped_count(), 2);
    }

    #[test]
    fn test_run_result_empty_checks() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![],
            duration: Duration::ZERO,
        };

        assert!(result.success());
        assert_eq!(result.passed_count(), 0);
        assert_eq!(result.failed_count(), 0);
        assert_eq!(result.skipped_count(), 0);
    }

    #[test]
    fn test_run_result_failed_checks_iterator() {
        let result = RunResult {
            mode: Mode::Agent,
            checks: vec![
                make_passed_check("pass1"),
                make_failed_check("fail1"),
                make_passed_check("pass2"),
                make_failed_check("fail2"),
            ],
            duration: Duration::ZERO,
        };

        let failed: Vec<_> = result.failed_checks().collect();
        assert_eq!(failed.len(), 2);
        assert_eq!(failed[0].name, "fail1");
        assert_eq!(failed[1].name, "fail2");
    }

    #[test]
    fn test_run_result_mode_preserved() {
        let human_result = RunResult {
            mode: Mode::Human,
            checks: vec![],
            duration: Duration::ZERO,
        };
        assert_eq!(human_result.mode, Mode::Human);

        let agent_result = RunResult {
            mode: Mode::Agent,
            checks: vec![],
            duration: Duration::ZERO,
        };
        assert_eq!(agent_result.mode, Mode::Agent);

        let ci_result = RunResult {
            mode: Mode::Ci,
            checks: vec![],
            duration: Duration::ZERO,
        };
        assert_eq!(ci_result.mode, Mode::Ci);
    }

    #[test]
    fn test_run_result_duration_preserved() {
        let result = RunResult {
            mode: Mode::Human,
            checks: vec![],
            duration: Duration::from_secs(42),
        };
        assert_eq!(result.duration, Duration::from_secs(42));
    }

    // =========================================================================
    // Runner tests
    // =========================================================================

    #[test]
    fn test_runner_new() {
        let config = Config::default();
        let runner = Runner::new(config);
        let debug_str = format!("{:?}", runner);
        assert!(debug_str.contains("Runner"));
    }

    #[test]
    fn test_runner_with_config() {
        let config = Config::for_preset("rust");
        let runner = Runner::new(config);
        let debug_str = format!("{:?}", runner);
        assert!(debug_str.contains("Runner"));
    }

    // =========================================================================
    // check_enabled tests
    // =========================================================================

    #[test]
    fn test_check_enabled_no_condition() {
        let check = CheckConfig {
            run: "echo test".to_string(),
            description: "test".to_string(),
            enabled_if: None,
            env: HashMap::new(),
        };
        assert!(check_enabled(&check, None));
    }

    #[test]
    fn test_check_enabled_with_empty_condition() {
        let check = CheckConfig {
            run: "echo test".to_string(),
            description: "test".to_string(),
            enabled_if: Some(crate::config::EnabledCondition::default()),
            env: HashMap::new(),
        };
        assert!(check_enabled(&check, None));
    }

    #[test]
    fn test_check_enabled_command_exists() {
        let check = CheckConfig {
            run: "echo test".to_string(),
            description: "test".to_string(),
            enabled_if: Some(crate::config::EnabledCondition {
                file_exists: None,
                dir_exists: None,
                command_exists: Some("sh".to_string()),
            }),
            env: HashMap::new(),
        };
        assert!(check_enabled(&check, None));
    }

    #[test]
    fn test_check_enabled_command_not_exists() {
        let check = CheckConfig {
            run: "echo test".to_string(),
            description: "test".to_string(),
            enabled_if: Some(crate::config::EnabledCondition {
                file_exists: None,
                dir_exists: None,
                command_exists: Some("definitely_not_a_command_12345".to_string()),
            }),
            env: HashMap::new(),
        };
        assert!(!check_enabled(&check, None));
    }

    // =========================================================================
    // concurrency tests
    // =========================================================================

    #[test]
    fn test_available_parallelism() {
        let parallelism = concurrency::available_parallelism();
        assert!(parallelism >= 1);
    }

    // =========================================================================
    // Security tests - timeout parsing
    // =========================================================================

    #[test]
    fn test_parse_duration_valid_formats() {
        // Verify that valid duration formats are parsed correctly
        assert!(parse_duration("30s").is_some());
        assert!(parse_duration("5m").is_some());
        assert!(parse_duration("1h").is_some());
        assert!(parse_duration("1h30m").is_some());
        assert!(parse_duration("15m30s").is_some());
    }

    #[test]
    fn test_parse_duration_invalid_returns_none() {
        // Invalid formats should return None (triggering the warning log)
        assert!(parse_duration("invalid").is_none());
        assert!(parse_duration("").is_none());
        assert!(parse_duration("abc").is_none());
        assert!(parse_duration("-5s").is_none());
        // Note: These now return None, which triggers the warning log in run_check_async
    }

    #[test]
    fn test_parse_duration_boundary_values() {
        // Test boundary values
        assert!(parse_duration("0s").is_some());
        assert!(parse_duration("1s").is_some());
        assert!(parse_duration("999999s").is_some());
    }
}
