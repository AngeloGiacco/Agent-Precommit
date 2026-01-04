//! Configuration handling for agent-precommit.
//!
//! This module provides configuration loading and validation,
//! supporting both `agent-precommit.toml` files and sensible defaults.

use crate::core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default configuration file name.
pub const CONFIG_FILE_NAME: &str = "agent-precommit.toml";

/// Main configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Detection settings.
    pub detection: DetectionConfig,
    /// Integration with other tools.
    pub integration: IntegrationConfig,
    /// Human mode settings.
    pub human: ModeConfig,
    /// Agent mode settings.
    pub agent: AgentModeConfig,
    /// Check definitions.
    #[serde(default)]
    pub checks: HashMap<String, CheckConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            detection: DetectionConfig::default(),
            integration: IntegrationConfig::default(),
            human: ModeConfig::default_human(),
            agent: AgentModeConfig::default(),
            checks: default_checks(),
        }
    }
}

impl Config {
    /// Loads configuration from the default location.
    pub fn load() -> Result<Self> {
        let path = Self::find_config_file()?;
        Self::load_from(&path)
    }

    /// Loads configuration or returns defaults if not found.
    pub fn load_or_default() -> Result<Self> {
        match Self::find_config_file() {
            Ok(path) => Self::load_from(&path),
            Err(Error::ConfigNotFound { .. }) => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Loads configuration from a specific path.
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| Error::io("read config", e))?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| Error::config_parse_with_source("Failed to parse TOML", e))?;

        config.validate()?;

        Ok(config)
    }

    /// Finds the configuration file by searching up the directory tree.
    pub fn find_config_file() -> Result<PathBuf> {
        let cwd = std::env::current_dir().map_err(|e| Error::io("get current dir", e))?;

        let mut current = cwd.as_path();
        loop {
            let config_path = current.join(CONFIG_FILE_NAME);
            if config_path.exists() {
                return Ok(config_path);
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        Err(Error::ConfigNotFound {
            path: cwd.join(CONFIG_FILE_NAME),
        })
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        // Validate timeouts are parseable
        if humantime::parse_duration(&self.human.timeout).is_err() {
            return Err(Error::ConfigInvalid {
                field: "human.timeout".to_string(),
                message: format!("Invalid duration: {}", self.human.timeout),
            });
        }

        if humantime::parse_duration(&self.agent.timeout).is_err() {
            return Err(Error::ConfigInvalid {
                field: "agent.timeout".to_string(),
                message: format!("Invalid duration: {}", self.agent.timeout),
            });
        }

        Ok(())
    }

    /// Generates default configuration as a string.
    #[must_use]
    pub fn default_toml() -> String {
        let config = Self::default();
        toml::to_string_pretty(&config).unwrap_or_default()
    }

    /// Generates configuration for a specific preset.
    #[must_use]
    pub fn for_preset(preset: &str) -> Self {
        let mut config = Self::default();

        match preset {
            "python" => {
                config.agent.checks = vec![
                    "pre-commit-all".to_string(),
                    "no-merge-conflicts".to_string(),
                    "test-unit".to_string(),
                    "test-integration".to_string(),
                    "security-scan".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(python_checks());
            }
            "node" | "nodejs" | "typescript" => {
                config.agent.checks = vec![
                    "pre-commit-all".to_string(),
                    "no-merge-conflicts".to_string(),
                    "lint".to_string(),
                    "typecheck".to_string(),
                    "test-unit".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(node_checks());
            }
            "rust" => {
                config.agent.checks = vec![
                    "no-merge-conflicts".to_string(),
                    "fmt-check".to_string(),
                    "clippy".to_string(),
                    "test-unit".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(rust_checks());
            }
            "go" => {
                config.agent.checks = vec![
                    "no-merge-conflicts".to_string(),
                    "fmt-check".to_string(),
                    "lint".to_string(),
                    "test-unit".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(go_checks());
            }
            _ => {}
        }

        config
    }
}

/// Detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DetectionConfig {
    /// Force a specific mode (overrides auto-detection).
    pub mode: Option<String>,
    /// Additional environment variables that indicate an agent.
    pub agent_env_vars: Vec<String>,
}

/// Integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationConfig {
    /// Enable pre-commit framework integration.
    pub pre_commit: bool,
    /// Path to pre-commit config file.
    pub pre_commit_path: String,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            pre_commit: false,
            pre_commit_path: ".pre-commit-config.yaml".to_string(),
        }
    }
}

/// Mode-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeConfig {
    /// Checks to run in this mode.
    pub checks: Vec<String>,
    /// Timeout for all checks.
    pub timeout: String,
    /// Whether to stop on first failure.
    pub fail_fast: bool,
}

impl ModeConfig {
    fn default_human() -> Self {
        Self {
            checks: vec!["pre-commit".to_string()],
            timeout: "30s".to_string(),
            fail_fast: true,
        }
    }
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self::default_human()
    }
}

/// Agent mode configuration with parallel execution support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentModeConfig {
    /// Checks to run in agent mode.
    pub checks: Vec<String>,
    /// Timeout for all checks.
    pub timeout: String,
    /// Whether to stop on first failure.
    pub fail_fast: bool,
    /// Groups of checks that can run in parallel.
    pub parallel_groups: Vec<Vec<String>>,
}

impl Default for AgentModeConfig {
    fn default() -> Self {
        Self {
            checks: vec![
                "pre-commit-all".to_string(),
                "no-merge-conflicts".to_string(),
                "test-unit".to_string(),
            ],
            timeout: "15m".to_string(),
            fail_fast: false,
            parallel_groups: Vec::new(),
        }
    }
}

/// Configuration for a single check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckConfig {
    /// Command to run.
    pub run: String,
    /// Human-readable description.
    pub description: String,
    /// Condition for enabling the check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_if: Option<EnabledCondition>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            run: String::new(),
            description: String::new(),
            enabled_if: None,
            env: HashMap::new(),
        }
    }
}

impl CheckConfig {
    /// Creates a check config from a simple command.
    #[must_use]
    pub fn from_command(cmd: String) -> Self {
        Self {
            run: cmd.clone(),
            description: cmd,
            enabled_if: None,
            env: HashMap::new(),
        }
    }
}

/// Condition for enabling a check.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EnabledCondition {
    /// Check if a file exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_exists: Option<String>,
    /// Check if a directory exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_exists: Option<String>,
    /// Check if a command exists in PATH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_exists: Option<String>,
}

/// Default checks for all configurations.
fn default_checks() -> HashMap<String, CheckConfig> {
    let mut checks = HashMap::new();

    checks.insert(
        "pre-commit".to_string(),
        CheckConfig {
            run: "pre-commit run".to_string(),
            description: "Run pre-commit on staged files".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some(".pre-commit-config.yaml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "pre-commit-all".to_string(),
        CheckConfig {
            run: "pre-commit run --all-files".to_string(),
            description: "Run pre-commit on all files".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some(".pre-commit-config.yaml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "no-merge-conflicts".to_string(),
        CheckConfig {
            run: r#"
git fetch origin main --quiet 2>/dev/null || git fetch origin master --quiet 2>/dev/null || true
MAIN_BRANCH=$(git rev-parse --verify origin/main 2>/dev/null && echo "main" || echo "master")
BASE=$(git merge-base HEAD origin/$MAIN_BRANCH 2>/dev/null || echo "")
if [ -n "$BASE" ]; then
    if git merge-tree $BASE HEAD origin/$MAIN_BRANCH 2>/dev/null | grep -q "^<<<<<<<"; then
        echo "❌ Would conflict with $MAIN_BRANCH"
        exit 1
    fi
fi
echo "✓ No conflicts with $MAIN_BRANCH"
"#
            .trim()
            .to_string(),
            description: "Ensure no merge conflicts with main/master".to_string(),
            enabled_if: None,
            env: HashMap::new(),
        },
    );

    checks
}

/// Python-specific checks.
fn python_checks() -> HashMap<String, CheckConfig> {
    let mut checks = HashMap::new();

    checks.insert(
        "test-unit".to_string(),
        CheckConfig {
            run: "pytest -x -q".to_string(),
            description: "Run unit tests".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("pyproject.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "test-integration".to_string(),
        CheckConfig {
            run: "pytest tests/integration/ -v".to_string(),
            description: "Run integration tests".to_string(),
            enabled_if: Some(EnabledCondition {
                dir_exists: Some("tests/integration".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "security-scan".to_string(),
        CheckConfig {
            run: "gitleaks detect --source . --no-git".to_string(),
            description: "Scan for secrets".to_string(),
            enabled_if: Some(EnabledCondition {
                command_exists: Some("gitleaks".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "build-verify".to_string(),
        CheckConfig {
            run: "python -m build --no-isolation".to_string(),
            description: "Verify package builds".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("pyproject.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks
}

/// Node.js/TypeScript checks.
fn node_checks() -> HashMap<String, CheckConfig> {
    let mut checks = HashMap::new();

    checks.insert(
        "lint".to_string(),
        CheckConfig {
            run: "npm run lint".to_string(),
            description: "Run ESLint".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("package.json".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "typecheck".to_string(),
        CheckConfig {
            run: "npm run typecheck || npx tsc --noEmit".to_string(),
            description: "Run TypeScript type checking".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("tsconfig.json".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "test-unit".to_string(),
        CheckConfig {
            run: "npm test".to_string(),
            description: "Run unit tests".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("package.json".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "build-verify".to_string(),
        CheckConfig {
            run: "npm run build".to_string(),
            description: "Verify build works".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("package.json".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks
}

/// Rust checks.
fn rust_checks() -> HashMap<String, CheckConfig> {
    let mut checks = HashMap::new();

    checks.insert(
        "fmt-check".to_string(),
        CheckConfig {
            run: "cargo fmt --all -- --check".to_string(),
            description: "Check code formatting".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("Cargo.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "clippy".to_string(),
        CheckConfig {
            run: "cargo clippy --all-targets --all-features -- -D warnings".to_string(),
            description: "Run Clippy lints".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("Cargo.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "test-unit".to_string(),
        CheckConfig {
            run: "cargo test".to_string(),
            description: "Run unit tests".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("Cargo.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "build-verify".to_string(),
        CheckConfig {
            run: "cargo build --release".to_string(),
            description: "Verify release build".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("Cargo.toml".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks
}

/// Go checks.
fn go_checks() -> HashMap<String, CheckConfig> {
    let mut checks = HashMap::new();

    checks.insert(
        "fmt-check".to_string(),
        CheckConfig {
            run: "test -z \"$(gofmt -l .)\"".to_string(),
            description: "Check code formatting".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("go.mod".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "lint".to_string(),
        CheckConfig {
            run: "golangci-lint run".to_string(),
            description: "Run golangci-lint".to_string(),
            enabled_if: Some(EnabledCondition {
                command_exists: Some("golangci-lint".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "test-unit".to_string(),
        CheckConfig {
            run: "go test ./...".to_string(),
            description: "Run unit tests".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("go.mod".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks.insert(
        "build-verify".to_string(),
        CheckConfig {
            run: "go build ./...".to_string(),
            description: "Verify build works".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("go.mod".to_string()),
                ..Default::default()
            }),
            env: HashMap::new(),
        },
    );

    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.human.checks.is_empty());
        assert!(!config.agent.checks.is_empty());
    }

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_timeout() {
        let mut config = Config::default();
        config.human.timeout = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_preset_python() {
        let config = Config::for_preset("python");
        assert!(config.checks.contains_key("test-unit"));
        assert!(config.checks.contains_key("build-verify"));
    }

    #[test]
    fn test_preset_rust() {
        let config = Config::for_preset("rust");
        assert!(config.checks.contains_key("clippy"));
        assert!(config.checks.contains_key("fmt-check"));
    }

    #[test]
    fn test_default_toml_generation() {
        let toml = Config::default_toml();
        assert!(!toml.is_empty());
        assert!(toml.contains("[human]"));
        assert!(toml.contains("[agent]"));
    }
}
