//! Git repository operations.
//!
//! This module provides utilities for interacting with Git repositories,
//! including finding the repository root, hooks directory, and staged files.

use crate::core::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a Git repository.
#[derive(Debug, Clone)]
pub struct GitRepo {
    /// Root directory of the repository (where .git is).
    root: PathBuf,
    /// Path to the .git directory (or file for worktrees).
    git_dir: PathBuf,
}

impl GitRepo {
    /// Discovers the Git repository from the current directory.
    pub fn discover() -> Result<Self> {
        Self::discover_from(&std::env::current_dir().map_err(|e| Error::io("get current dir", e))?)
    }

    /// Discovers the Git repository from a specific path.
    pub fn discover_from(path: &Path) -> Result<Self> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel", "--git-dir"])
            .current_dir(path)
            .output()
            .map_err(|e| Error::io("run git rev-parse", e))?;

        if !output.status.success() {
            return Err(Error::NotGitRepo);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();

        let root = lines
            .next()
            .map(PathBuf::from)
            .ok_or(Error::NotGitRepo)?;

        let git_dir = lines
            .next()
            .map(|s| {
                let p = PathBuf::from(s);
                if p.is_absolute() {
                    p
                } else {
                    root.join(p)
                }
            })
            .ok_or(Error::NotGitRepo)?;

        Ok(Self { root, git_dir })
    }

    /// Returns the root directory of the repository.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the .git directory path.
    #[must_use]
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Returns the hooks directory path.
    #[must_use]
    pub fn hooks_dir(&self) -> PathBuf {
        // Check for custom hooks path first
        if let Ok(output) = Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(&self.root)
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    let hooks_path = PathBuf::from(&path);
                    if hooks_path.is_absolute() {
                        return hooks_path;
                    }
                    return self.root.join(hooks_path);
                }
            }
        }

        // Default to .git/hooks
        self.git_dir.join("hooks")
    }

    /// Returns the path to a specific hook.
    #[must_use]
    pub fn hook_path(&self, hook_name: &str) -> PathBuf {
        self.hooks_dir().join(hook_name)
    }

    /// Returns the list of staged files.
    pub fn staged_files(&self) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("get staged files", e))?;

        if !output.status.success() {
            return Err(Error::git("diff --cached", "Failed to get staged files"));
        }

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| self.root.join(s))
            .collect();

        Ok(files)
    }

    /// Returns the current branch name.
    pub fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("get current branch", e))?;

        if !output.status.success() {
            return Err(Error::git("rev-parse", "Failed to get current branch"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Returns the main branch name (main or master).
    pub fn main_branch(&self) -> Result<String> {
        // Try 'main' first
        let output = Command::new("git")
            .args(["rev-parse", "--verify", "origin/main"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("verify main branch", e))?;

        if output.status.success() {
            return Ok("main".to_string());
        }

        // Fall back to 'master'
        let output = Command::new("git")
            .args(["rev-parse", "--verify", "origin/master"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("verify master branch", e))?;

        if output.status.success() {
            return Ok("master".to_string());
        }

        // Default to 'main' if neither exists
        Ok("main".to_string())
    }

    /// Fetches updates from the remote for a specific branch.
    pub fn fetch_branch(&self, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["fetch", "origin", branch, "--quiet"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("fetch branch", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::git("fetch", stderr.trim().to_string()));
        }

        Ok(())
    }

    /// Checks if the repository has uncommitted changes.
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::io("check uncommitted changes", e))?;

        if !output.status.success() {
            return Err(Error::git("status", "Failed to check status"));
        }

        Ok(!output.stdout.is_empty())
    }

    /// Checks if a file exists in the repository.
    #[must_use]
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.root.join(relative_path).exists()
    }

    /// Checks if a directory exists in the repository.
    #[must_use]
    pub fn dir_exists(&self, relative_path: &str) -> bool {
        self.root.join(relative_path).is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, GitRepo) {
        let temp = TempDir::new().expect("create temp dir");
        let path = temp.path();

        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .expect("init repo");

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .expect("set email");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .expect("set name");

        let repo = GitRepo::discover_from(path).expect("discover repo");
        (temp, repo)
    }

    #[test]
    fn test_discover_repo() {
        let (_temp, repo) = create_test_repo();
        assert!(repo.root().exists());
        assert!(repo.git_dir().exists());
    }

    #[test]
    fn test_hooks_dir() {
        let (_temp, repo) = create_test_repo();
        let hooks_dir = repo.hooks_dir();
        assert!(hooks_dir.ends_with("hooks"));
    }

    #[test]
    fn test_not_git_repo() {
        let temp = TempDir::new().expect("create temp dir");
        let result = GitRepo::discover_from(temp.path());
        assert!(matches!(result, Err(Error::NotGitRepo)));
    }
}
