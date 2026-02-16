//! Error types for agent-precommit.
//!
//! This module defines all errors that can occur during operation.

use std::path::PathBuf;

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// All possible errors in agent-precommit.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // =========================================================================
    // Configuration errors
    // =========================================================================
    /// Configuration file not found.
    #[error("Configuration file not found: {path}")]
    ConfigNotFound {
        /// Path where config was expected.
        path: PathBuf,
    },

    /// Failed to parse configuration file.
    #[error("Failed to parse configuration: {message}")]
    ConfigParse {
        /// Description of the parse error.
        message: String,
        /// Optional source error.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Invalid configuration value.
    #[error("Invalid configuration: {field} - {message}")]
    ConfigInvalid {
        /// Field name that is invalid.
        field: String,
        /// Description of why it's invalid.
        message: String,
    },

    // =========================================================================
    // Git errors
    // =========================================================================
    /// Not in a Git repository.
    #[error("Not in a Git repository")]
    NotGitRepo,

    /// Git operation failed.
    #[error("Git operation failed: {operation} - {message}")]
    GitOperation {
        /// Name of the operation that failed.
        operation: String,
        /// Error message.
        message: String,
    },

    /// Failed to detect Git hooks directory.
    #[error("Failed to detect Git hooks directory")]
    GitHooksDir,

    // =========================================================================
    // Check execution errors
    // =========================================================================
    /// Check not found.
    #[error("Check not found: {name}")]
    CheckNotFound {
        /// Name of the check that wasn't found.
        name: String,
    },

    /// Check execution failed.
    #[error("Check '{name}' failed: {message}")]
    CheckFailed {
        /// Name of the check that failed.
        name: String,
        /// Error message or output.
        message: String,
        /// Exit code if available.
        exit_code: Option<i32>,
    },

    /// Check timed out.
    #[error("Check '{name}' timed out after {timeout}")]
    CheckTimeout {
        /// Name of the check that timed out.
        name: String,
        /// Timeout duration as string.
        timeout: String,
    },

    /// Command not found.
    #[error("Command not found: {command}")]
    CommandNotFound {
        /// The command that wasn't found.
        command: String,
    },

    // =========================================================================
    // Hook errors
    // =========================================================================
    /// Failed to install hook.
    #[error("Failed to install Git hook: {message}")]
    HookInstall {
        /// Error message.
        message: String,
    },

    /// Hook already exists and wasn't created by us.
    #[error("Git hook already exists at {path}. Use --force to overwrite.")]
    HookExists {
        /// Path to existing hook.
        path: PathBuf,
    },

    // =========================================================================
    // I/O errors
    // =========================================================================
    /// File I/O error.
    #[error("I/O error: {message}")]
    Io {
        /// Description of what failed.
        message: String,
        /// Source error.
        #[source]
        source: std::io::Error,
    },

    // =========================================================================
    // Pre-commit integration errors
    // =========================================================================
    /// Pre-commit framework not found.
    #[error("Pre-commit framework not found. Install with: pip install pre-commit")]
    PreCommitNotFound,

    /// Pre-commit config not found.
    #[error("Pre-commit config not found: {path}")]
    PreCommitConfigNotFound {
        /// Path where config was expected.
        path: PathBuf,
    },

    // =========================================================================
    // Internal errors
    // =========================================================================
    /// Internal error (should never happen).
    #[error("Internal error: {message}")]
    Internal {
        /// Error message.
        message: String,
    },
}

impl Error {
    /// Creates a new configuration parse error.
    pub fn config_parse(message: impl Into<String>) -> Self {
        Self::ConfigParse {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a new configuration parse error with source.
    pub fn config_parse_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::ConfigParse {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a new I/O error with context.
    pub fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source,
        }
    }

    /// Creates a new Git operation error.
    pub fn git(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::GitOperation {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Creates a new check failed error.
    pub fn check_failed(
        name: impl Into<String>,
        message: impl Into<String>,
        exit_code: Option<i32>,
    ) -> Self {
        Self::CheckFailed {
            name: name.into(),
            message: message.into(),
            exit_code,
        }
    }

    /// Returns true if this is a user-correctable error.
    pub const fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::ConfigNotFound { .. }
                | Self::ConfigInvalid { .. }
                | Self::NotGitRepo
                | Self::HookExists { .. }
                | Self::PreCommitNotFound
                | Self::PreCommitConfigNotFound { .. }
        )
    }

    /// Returns an exit code appropriate for this error.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::CheckFailed { exit_code, .. } => exit_code.unwrap_or(1),
            Self::CheckTimeout { .. } => 124, // Standard timeout exit code
            Self::ConfigNotFound { .. } | Self::ConfigParse { .. } | Self::ConfigInvalid { .. } => {
                78
            }, // EX_CONFIG
            Self::NotGitRepo | Self::GitOperation { .. } | Self::GitHooksDir => 65, // EX_DATAERR
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Display / Error message tests for every variant
    // =========================================================================

    #[test]
    fn test_display_config_not_found() {
        let err = Error::ConfigNotFound {
            path: PathBuf::from("/my/config.toml"),
        };
        assert_eq!(
            err.to_string(),
            "Configuration file not found: /my/config.toml"
        );
    }

    #[test]
    fn test_display_config_parse() {
        let err = Error::config_parse("bad toml syntax");
        assert_eq!(
            err.to_string(),
            "Failed to parse configuration: bad toml syntax"
        );
    }

    #[test]
    fn test_display_config_invalid() {
        let err = Error::ConfigInvalid {
            field: "human.timeout".to_string(),
            message: "Invalid duration".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid configuration: human.timeout - Invalid duration"
        );
    }

    #[test]
    fn test_display_not_git_repo() {
        let err = Error::NotGitRepo;
        assert_eq!(err.to_string(), "Not in a Git repository");
    }

    #[test]
    fn test_display_git_operation() {
        let err = Error::git("fetch", "network error");
        assert_eq!(
            err.to_string(),
            "Git operation failed: fetch - network error"
        );
    }

    #[test]
    fn test_display_git_hooks_dir() {
        let err = Error::GitHooksDir;
        assert_eq!(err.to_string(), "Failed to detect Git hooks directory");
    }

    #[test]
    fn test_display_check_not_found() {
        let err = Error::CheckNotFound {
            name: "test-lint".to_string(),
        };
        assert_eq!(err.to_string(), "Check not found: test-lint");
    }

    #[test]
    fn test_display_check_failed() {
        let err = Error::check_failed("test-unit", "assertion failed", Some(1));
        assert_eq!(
            err.to_string(),
            "Check 'test-unit' failed: assertion failed"
        );
    }

    #[test]
    fn test_display_check_timeout() {
        let err = Error::CheckTimeout {
            name: "slow-test".to_string(),
            timeout: "30s".to_string(),
        };
        assert_eq!(err.to_string(), "Check 'slow-test' timed out after 30s");
    }

    #[test]
    fn test_display_command_not_found() {
        let err = Error::CommandNotFound {
            command: "cargo".to_string(),
        };
        assert_eq!(err.to_string(), "Command not found: cargo");
    }

    #[test]
    fn test_display_hook_install() {
        let err = Error::HookInstall {
            message: "permission denied".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to install Git hook: permission denied"
        );
    }

    #[test]
    fn test_display_hook_exists() {
        let err = Error::HookExists {
            path: PathBuf::from(".git/hooks/pre-commit"),
        };
        assert_eq!(
            err.to_string(),
            "Git hook already exists at .git/hooks/pre-commit. Use --force to overwrite."
        );
    }

    #[test]
    fn test_display_io() {
        let err = Error::io("read config", std::io::Error::other("file not found"));
        assert_eq!(err.to_string(), "I/O error: read config");
    }

    #[test]
    fn test_display_precommit_not_found() {
        let err = Error::PreCommitNotFound;
        assert_eq!(
            err.to_string(),
            "Pre-commit framework not found. Install with: pip install pre-commit"
        );
    }

    #[test]
    fn test_display_precommit_config_not_found() {
        let err = Error::PreCommitConfigNotFound {
            path: PathBuf::from(".pre-commit-config.yaml"),
        };
        assert_eq!(
            err.to_string(),
            "Pre-commit config not found: .pre-commit-config.yaml"
        );
    }

    #[test]
    fn test_display_internal() {
        let err = Error::Internal {
            message: "unexpected state".to_string(),
        };
        assert_eq!(err.to_string(), "Internal error: unexpected state");
    }

    // =========================================================================
    // Constructor tests
    // =========================================================================

    #[test]
    fn test_config_parse_no_source() {
        let err = Error::config_parse("bad syntax");
        assert!(matches!(&err, Error::ConfigParse { message, source }
            if message == "bad syntax" && source.is_none()
        ));
    }

    #[test]
    fn test_config_parse_with_source() {
        let toml_err = toml::from_str::<toml::Value>("invalid [[[toml").expect_err("should fail");
        let err = Error::config_parse_with_source("bad toml", toml_err);
        assert!(matches!(&err, Error::ConfigParse { message, source }
            if message == "bad toml" && source.is_some()
        ));
    }

    #[test]
    fn test_io_constructor() {
        let io_err = std::io::Error::other("denied");
        let err = Error::io("write file", io_err);
        assert!(matches!(&err, Error::Io { message, .. } if message == "write file"));
    }

    #[test]
    fn test_git_constructor() {
        let err = Error::git("merge", "conflict detected");
        assert!(matches!(&err, Error::GitOperation { operation, message }
            if operation == "merge" && message == "conflict detected"
        ));
    }

    #[test]
    fn test_check_failed_with_exit_code() {
        let err = Error::check_failed("lint", "style error", Some(2));
        assert!(
            matches!(&err, Error::CheckFailed { name, message, exit_code }
                if name == "lint" && message == "style error" && *exit_code == Some(2)
            )
        );
    }

    #[test]
    fn test_check_failed_without_exit_code() {
        let err = Error::check_failed("lint", "killed", None);
        assert!(matches!(&err, Error::CheckFailed { exit_code, .. }
            if exit_code.is_none()
        ));
    }

    // =========================================================================
    // Exit code tests for all variants
    // =========================================================================

    #[test]
    fn test_exit_code_check_failed_with_code() {
        assert_eq!(Error::check_failed("t", "m", Some(42)).exit_code(), 42);
    }

    #[test]
    fn test_exit_code_check_failed_without_code() {
        assert_eq!(Error::check_failed("t", "m", None).exit_code(), 1);
    }

    #[test]
    fn test_exit_code_check_timeout() {
        assert_eq!(
            Error::CheckTimeout {
                name: "t".into(),
                timeout: "30s".into(),
            }
            .exit_code(),
            124
        );
    }

    #[test]
    fn test_exit_code_config_not_found() {
        assert_eq!(
            Error::ConfigNotFound {
                path: PathBuf::from("x")
            }
            .exit_code(),
            78
        );
    }

    #[test]
    fn test_exit_code_config_parse() {
        assert_eq!(Error::config_parse("x").exit_code(), 78);
    }

    #[test]
    fn test_exit_code_config_invalid() {
        assert_eq!(
            Error::ConfigInvalid {
                field: "x".into(),
                message: "y".into()
            }
            .exit_code(),
            78
        );
    }

    #[test]
    fn test_exit_code_not_git_repo() {
        assert_eq!(Error::NotGitRepo.exit_code(), 65);
    }

    #[test]
    fn test_exit_code_git_operation() {
        assert_eq!(Error::git("op", "msg").exit_code(), 65);
    }

    #[test]
    fn test_exit_code_git_hooks_dir() {
        assert_eq!(Error::GitHooksDir.exit_code(), 65);
    }

    #[test]
    fn test_exit_code_internal() {
        assert_eq!(
            Error::Internal {
                message: "x".into()
            }
            .exit_code(),
            1
        );
    }

    #[test]
    fn test_exit_code_check_not_found() {
        assert_eq!(Error::CheckNotFound { name: "x".into() }.exit_code(), 1);
    }

    #[test]
    fn test_exit_code_hook_exists() {
        assert_eq!(
            Error::HookExists {
                path: PathBuf::from("x")
            }
            .exit_code(),
            1
        );
    }

    // =========================================================================
    // is_user_error tests for all variants
    // =========================================================================

    #[test]
    fn test_is_user_error_config_not_found() {
        assert!(Error::ConfigNotFound {
            path: PathBuf::from("x")
        }
        .is_user_error());
    }

    #[test]
    fn test_is_user_error_config_invalid() {
        assert!(Error::ConfigInvalid {
            field: "x".into(),
            message: "y".into()
        }
        .is_user_error());
    }

    #[test]
    fn test_is_user_error_not_git_repo() {
        assert!(Error::NotGitRepo.is_user_error());
    }

    #[test]
    fn test_is_user_error_hook_exists() {
        assert!(Error::HookExists {
            path: PathBuf::from("x")
        }
        .is_user_error());
    }

    #[test]
    fn test_is_user_error_precommit_not_found() {
        assert!(Error::PreCommitNotFound.is_user_error());
    }

    #[test]
    fn test_is_user_error_precommit_config_not_found() {
        assert!(Error::PreCommitConfigNotFound {
            path: PathBuf::from("x")
        }
        .is_user_error());
    }

    #[test]
    fn test_is_not_user_error_config_parse() {
        assert!(!Error::config_parse("x").is_user_error());
    }

    #[test]
    fn test_is_not_user_error_git_operation() {
        assert!(!Error::git("op", "msg").is_user_error());
    }

    #[test]
    fn test_is_not_user_error_check_failed() {
        assert!(!Error::check_failed("x", "y", None).is_user_error());
    }

    #[test]
    fn test_is_not_user_error_check_timeout() {
        assert!(!Error::CheckTimeout {
            name: "x".into(),
            timeout: "30s".into()
        }
        .is_user_error());
    }

    #[test]
    fn test_is_not_user_error_internal() {
        assert!(!Error::Internal {
            message: "x".into()
        }
        .is_user_error());
    }

    #[test]
    fn test_is_not_user_error_io() {
        assert!(!Error::io("x", std::io::Error::other("y")).is_user_error());
    }

    // =========================================================================
    // Error source chain tests
    // =========================================================================

    #[test]
    fn test_io_error_has_source() {
        use std::error::Error as StdError;
        let err = Error::io("x", std::io::Error::other("inner"));
        assert!(err.source().is_some());
    }

    #[test]
    fn test_config_parse_with_source_has_source() {
        use std::error::Error as StdError;
        let toml_err = toml::from_str::<toml::Value>("bad").expect_err("should fail");
        let err = Error::config_parse_with_source("msg", toml_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_config_parse_without_source_has_no_source() {
        use std::error::Error as StdError;
        let err = Error::config_parse("msg");
        assert!(err.source().is_none());
    }

    // =========================================================================
    // Debug trait test
    // =========================================================================

    #[test]
    fn test_error_debug() {
        let err = Error::NotGitRepo;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotGitRepo"));
    }
}
