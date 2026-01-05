//! Pre-commit framework integration.
//!
//! This module provides integration with the pre-commit framework.

use crate::core::error::{Error, Result};
use crate::core::executor::{ExecuteOptions, Executor};
use std::path::Path;

/// Path to the pre-commit config file.
pub const PRE_COMMIT_CONFIG: &str = ".pre-commit-config.yaml";

/// Checks if pre-commit is installed.
pub fn is_installed() -> bool {
    Executor::command_exists("pre-commit")
}

/// Checks if a pre-commit config exists.
pub fn config_exists(repo_root: &Path) -> bool {
    repo_root.join(PRE_COMMIT_CONFIG).exists()
}

/// Runs pre-commit on staged files.
pub async fn run_staged(repo_root: &Path) -> Result<bool> {
    run_with_args(repo_root, &[]).await
}

/// Runs pre-commit on all files.
pub async fn run_all(repo_root: &Path) -> Result<bool> {
    run_with_args(repo_root, &["--all-files"]).await
}

/// Runs pre-commit with custom arguments.
///
/// # Security
///
/// This function only accepts arguments from hardcoded sources within this crate
/// (e.g., `&["--all-files"]`). Arguments are validated to ensure they start with
/// `-` to prevent command injection if this function is ever called with
/// user-controlled input in the future.
async fn run_with_args(repo_root: &Path, args: &[&str]) -> Result<bool> {
    if !is_installed() {
        return Err(Error::PreCommitNotFound);
    }

    if !config_exists(repo_root) {
        return Err(Error::PreCommitConfigNotFound {
            path: repo_root.join(PRE_COMMIT_CONFIG),
        });
    }

    // Security: Validate that all arguments look like flags (start with -)
    // This prevents potential command injection if args were ever user-controlled
    for arg in args {
        if !arg.starts_with('-') {
            return Err(Error::Internal {
                message: format!("Invalid pre-commit argument: {arg}"),
            });
        }
    }

    let cmd = if args.is_empty() {
        "pre-commit run".to_string()
    } else {
        format!("pre-commit run {}", args.join(" "))
    };

    let executor = Executor::new();
    let output = executor
        .execute(
            &cmd,
            ExecuteOptions::default()
                .cwd(repo_root)
                .capture_output(false),
        )
        .await?;

    Ok(output.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Constants tests
    // =========================================================================

    #[test]
    fn test_pre_commit_config_constant() {
        assert_eq!(PRE_COMMIT_CONFIG, ".pre-commit-config.yaml");
    }

    // =========================================================================
    // config_exists tests
    // =========================================================================

    #[test]
    fn test_config_exists_no_file() {
        let temp = TempDir::new().expect("create temp dir");
        assert!(!config_exists(temp.path()));
    }

    #[test]
    fn test_config_exists_with_file() {
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");
        assert!(config_exists(temp.path()));
    }

    #[test]
    fn test_config_exists_empty_file() {
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "").expect("write empty config");
        assert!(config_exists(temp.path()));
    }

    #[test]
    fn test_config_exists_valid_yaml() {
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(
            temp.path().join(PRE_COMMIT_CONFIG),
            r#"
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.4.0
    hooks:
      - id: trailing-whitespace
"#,
        )
        .expect("write yaml config");
        assert!(config_exists(temp.path()));
    }

    // =========================================================================
    // is_installed tests
    // =========================================================================

    #[test]
    fn test_is_installed_returns_bool() {
        // This test just verifies the function runs without error
        // Result depends on whether pre-commit is installed on the system
        let _ = is_installed();
    }

    // =========================================================================
    // run_with_args tests (via async tests)
    // =========================================================================

    #[tokio::test]
    async fn test_run_staged_without_precommit() {
        // Skip this test if pre-commit is installed
        if is_installed() {
            return;
        }

        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        let result = run_staged(temp.path()).await;
        assert!(result.is_err());
        let err = result.expect_err("should return PreCommitNotFound");
        assert!(matches!(err, Error::PreCommitNotFound));
    }

    #[tokio::test]
    async fn test_run_all_without_precommit() {
        // Skip this test if pre-commit is installed
        if is_installed() {
            return;
        }

        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        let result = run_all(temp.path()).await;
        assert!(result.is_err());
        let err = result.expect_err("should return PreCommitNotFound");
        assert!(matches!(err, Error::PreCommitNotFound));
    }

    #[tokio::test]
    async fn test_run_staged_without_config() {
        // This test requires pre-commit to be installed
        if !is_installed() {
            return;
        }

        let temp = TempDir::new().expect("create temp dir");
        // Don't create config file

        let result = run_staged(temp.path()).await;
        assert!(result.is_err());
        let err = result.expect_err("should return PreCommitConfigNotFound");
        assert!(matches!(err, Error::PreCommitConfigNotFound { .. }));
    }

    #[tokio::test]
    async fn test_run_all_without_config() {
        // This test requires pre-commit to be installed
        if !is_installed() {
            return;
        }

        let temp = TempDir::new().expect("create temp dir");
        // Don't create config file

        let result = run_all(temp.path()).await;
        assert!(result.is_err());
        let err = result.expect_err("should return PreCommitConfigNotFound");
        assert!(matches!(err, Error::PreCommitConfigNotFound { .. }));
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_config_exists_in_nested_dir() {
        let temp = TempDir::new().expect("create temp dir");
        let nested = temp.path().join("nested/dir");
        std::fs::create_dir_all(&nested).expect("create nested dir");

        // Config in nested dir
        std::fs::write(nested.join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        // Should find config in nested, not in root
        assert!(!config_exists(temp.path()));
        assert!(config_exists(&nested));
    }

    #[test]
    fn test_config_path_construction() {
        let temp = TempDir::new().expect("create temp dir");
        let expected_path = temp.path().join(PRE_COMMIT_CONFIG);
        assert!(expected_path.ends_with(".pre-commit-config.yaml"));
    }

    // =========================================================================
    // Security tests - argument validation
    // =========================================================================

    #[tokio::test]
    async fn test_run_with_args_rejects_non_flag_arguments() {
        // This test verifies that non-flag arguments are rejected
        // to prevent potential command injection
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        // Skip if pre-commit is not installed - the validation happens before execution
        if !is_installed() {
            return;
        }

        // Test with a potentially malicious argument that doesn't start with -
        let result = run_with_args(temp.path(), &["--all-files", "; echo pwned"]).await;
        assert!(result.is_err());
        let err = result.expect_err("should reject non-flag argument");
        assert!(matches!(err, Error::Internal { .. }));
    }

    #[tokio::test]
    async fn test_run_with_args_accepts_valid_flags() {
        // This test verifies that valid flag arguments are accepted
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        // Skip if pre-commit is not installed
        if !is_installed() {
            return;
        }

        // Valid flags should pass validation (execution may still fail, but validation passes)
        // We can't fully test this without pre-commit being properly configured,
        // but the validation step should pass for valid flags
        let result = run_with_args(temp.path(), &["--all-files"]).await;
        // Result could be Ok or Err depending on pre-commit execution,
        // but it should NOT be an Internal error from validation
        if let Err(ref e) = result {
            assert!(
                !matches!(e, Error::Internal { .. }),
                "Valid flags should not cause validation error"
            );
        }
    }

    #[tokio::test]
    async fn test_run_with_args_rejects_path_injection() {
        // Test that path-like arguments are rejected
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        if !is_installed() {
            return;
        }

        let result = run_with_args(temp.path(), &["/etc/passwd"]).await;
        assert!(result.is_err());
        let err = result.expect_err("should reject path injection");
        assert!(matches!(err, Error::Internal { .. }));
    }

    #[tokio::test]
    async fn test_run_with_args_empty_is_valid() {
        // Empty args should be valid
        let temp = TempDir::new().expect("create temp dir");
        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []").expect("write config");

        if !is_installed() {
            return;
        }

        // Empty args should pass validation
        let result = run_with_args(temp.path(), &[]).await;
        // Result could be Ok or Err depending on pre-commit execution,
        // but it should NOT be an Internal error from validation
        if let Err(ref e) = result {
            assert!(
                !matches!(e, Error::Internal { .. }),
                "Empty args should not cause validation error"
            );
        }
    }
}
