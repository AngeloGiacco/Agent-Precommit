//! Built-in check definitions.
//!
//! These checks are available by default in all configurations.

/// Names of built-in checks.
pub mod names {
    /// Run pre-commit on staged files.
    pub const PRE_COMMIT: &str = "pre-commit";
    /// Run pre-commit on all files.
    pub const PRE_COMMIT_ALL: &str = "pre-commit-all";
    /// Check for merge conflicts with main/master.
    pub const NO_MERGE_CONFLICTS: &str = "no-merge-conflicts";
    /// Run unit tests.
    pub const TEST_UNIT: &str = "test-unit";
    /// Run integration tests.
    pub const TEST_INTEGRATION: &str = "test-integration";
    /// Scan for secrets.
    pub const SECURITY_SCAN: &str = "security-scan";
    /// Verify build works.
    pub const BUILD_VERIFY: &str = "build-verify";
}

/// Returns true if a check name is a built-in check.
#[must_use]
pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        names::PRE_COMMIT
            | names::PRE_COMMIT_ALL
            | names::NO_MERGE_CONFLICTS
            | names::TEST_UNIT
            | names::TEST_INTEGRATION
            | names::SECURITY_SCAN
            | names::BUILD_VERIFY
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("pre-commit"));
        assert!(is_builtin("no-merge-conflicts"));
        assert!(!is_builtin("custom-check"));
    }
}
