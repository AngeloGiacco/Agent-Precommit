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
async fn run_with_args(repo_root: &Path, args: &[&str]) -> Result<bool> {
    if !is_installed() {
        return Err(Error::PreCommitNotFound);
    }

    if !config_exists(repo_root) {
        return Err(Error::PreCommitConfigNotFound {
            path: repo_root.join(PRE_COMMIT_CONFIG),
        });
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

    #[test]
    fn test_config_exists() {
        let temp = TempDir::new().expect("create temp dir");
        assert!(!config_exists(temp.path()));

        std::fs::write(temp.path().join(PRE_COMMIT_CONFIG), "repos: []")
            .expect("write config");
        assert!(config_exists(temp.path()));
    }
}
