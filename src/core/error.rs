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
            Self::ConfigNotFound { .. }
            | Self::ConfigParse { .. }
            | Self::ConfigInvalid { .. } => 78, // EX_CONFIG
            Self::NotGitRepo | Self::GitOperation { .. } | Self::GitHooksDir => 65, // EX_DATAERR
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::CheckNotFound {
            name: "test".to_string(),
        };
        assert_eq!(err.to_string(), "Check not found: test");
    }

    #[test]
    fn test_exit_codes() {
        assert_eq!(
            Error::CheckTimeout {
                name: "test".into(),
                timeout: "30s".into()
            }
            .exit_code(),
            124
        );

        assert_eq!(
            Error::ConfigNotFound {
                path: PathBuf::from("/test")
            }
            .exit_code(),
            78
        );
    }

    #[test]
    fn test_is_user_error() {
        assert!(Error::NotGitRepo.is_user_error());
        assert!(Error::PreCommitNotFound.is_user_error());
        assert!(!Error::Internal {
            message: "test".into()
        }
        .is_user_error());
    }
}
