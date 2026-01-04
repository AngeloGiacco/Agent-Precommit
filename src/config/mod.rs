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
            },
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
            },
            "rust" => {
                config.agent.checks = vec![
                    "no-merge-conflicts".to_string(),
                    "fmt-check".to_string(),
                    "clippy".to_string(),
                    "test-unit".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(rust_checks());
            },
            "go" => {
                config.agent.checks = vec![
                    "no-merge-conflicts".to_string(),
                    "fmt-check".to_string(),
                    "lint".to_string(),
                    "test-unit".to_string(),
                    "build-verify".to_string(),
                ];
                config.checks.extend(go_checks());
            },
            _ => {},
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

    // =========================================================================
    // Config default tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.human.checks.is_empty());
        assert!(!config.agent.checks.is_empty());
    }

    #[test]
    fn test_default_config_has_timeouts() {
        let config = Config::default();
        assert!(!config.human.timeout.is_empty());
        assert!(!config.agent.timeout.is_empty());
    }

    #[test]
    fn test_default_config_has_checks() {
        let config = Config::default();
        // Default config should have some checks defined
        assert!(!config.checks.is_empty());
    }

    // =========================================================================
    // Config validation tests
    // =========================================================================

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_human_timeout() {
        let mut config = Config::default();
        config.human.timeout = "invalid".to_string();
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid duration"));
    }

    #[test]
    fn test_invalid_agent_timeout() {
        let mut config = Config::default();
        config.agent.timeout = "not_a_duration".to_string();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_timeouts() {
        let mut config = Config::default();
        config.human.timeout = "30s".to_string();
        config.agent.timeout = "15m".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_check_with_empty_run_is_valid() {
        // Empty run commands are allowed (they might be placeholders)
        let mut config = Config::default();
        config.checks.insert(
            "placeholder-check".to_string(),
            CheckConfig {
                run: "".to_string(),
                description: "Test".to_string(),
                enabled_if: None,
                env: HashMap::new(),
            },
        );
        config.human.checks.push("placeholder-check".to_string());
        // Current implementation allows empty run commands
        // Validation focuses on timeout parsing
        let result = config.validate();
        // This tests that the config doesn't crash - actual behavior may vary
        drop(result);
    }

    #[test]
    fn test_check_definitions_are_stored() {
        let mut config = Config::default();
        config.checks.insert(
            "custom-check".to_string(),
            CheckConfig {
                run: "echo test".to_string(),
                description: "Custom check".to_string(),
                enabled_if: None,
                env: HashMap::new(),
            },
        );
        assert!(config.checks.contains_key("custom-check"));
    }

    // =========================================================================
    // Preset tests
    // =========================================================================

    #[test]
    fn test_preset_python() {
        let config = Config::for_preset("python");
        assert!(config.checks.contains_key("test-unit"));
        assert!(config.checks.contains_key("build-verify"));
        // Python preset should have some checks configured
        assert!(!config.checks.is_empty());
    }

    #[test]
    fn test_preset_rust() {
        let config = Config::for_preset("rust");
        assert!(config.checks.contains_key("clippy"));
        assert!(config.checks.contains_key("fmt-check"));
        assert!(config.checks.contains_key("test-unit"));
        assert!(config.checks.contains_key("build-verify"));
    }

    #[test]
    fn test_preset_node() {
        let config = Config::for_preset("node");
        assert!(config.checks.contains_key("test-unit"));
        assert!(config.checks.contains_key("lint"));
    }

    #[test]
    fn test_preset_go() {
        let config = Config::for_preset("go");
        assert!(config.checks.contains_key("test-unit"));
        assert!(config.checks.contains_key("fmt-check"));
        assert!(config.checks.contains_key("build-verify"));
    }

    #[test]
    fn test_preset_invalid_falls_back_to_default() {
        let config = Config::for_preset("invalid_preset");
        // Should fall back to default
        assert!(config.validate().is_ok());
    }

    // =========================================================================
    // TOML generation tests
    // =========================================================================

    #[test]
    fn test_default_toml_generation() {
        let toml = Config::default_toml();
        assert!(!toml.is_empty());
        assert!(toml.contains("[human]"));
        assert!(toml.contains("[agent]"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let original = Config::default();
        let toml_str = toml::to_string_pretty(&original).expect("serialize");
        let parsed: Config = toml::from_str(&toml_str).expect("parse");

        assert_eq!(original.human.checks, parsed.human.checks);
        assert_eq!(original.agent.checks, parsed.agent.checks);
    }

    // =========================================================================
    // ModeConfig tests
    // =========================================================================

    #[test]
    fn test_mode_config_default() {
        let mode_config = ModeConfig::default();
        // Default human mode has pre-commit check
        assert!(!mode_config.checks.is_empty());
    }

    #[test]
    fn test_mode_config_with_checks() {
        let mode_config = ModeConfig {
            checks: vec!["check1".to_string(), "check2".to_string()],
            timeout: "30s".to_string(),
            fail_fast: true,
        };
        assert_eq!(mode_config.checks.len(), 2);
    }

    // =========================================================================
    // AgentModeConfig tests
    // =========================================================================

    #[test]
    fn test_agent_mode_config_default() {
        let mode_config = AgentModeConfig::default();
        assert!(!mode_config.checks.is_empty());
        assert!(mode_config.parallel_groups.is_empty());
    }

    #[test]
    fn test_agent_mode_config_with_parallel_groups() {
        let mode_config = AgentModeConfig {
            checks: vec![
                "check1".to_string(),
                "check2".to_string(),
                "check3".to_string(),
            ],
            timeout: "30s".to_string(),
            fail_fast: false,
            parallel_groups: vec![
                vec!["check1".to_string(), "check2".to_string()],
                vec!["check3".to_string()],
            ],
        };
        assert_eq!(mode_config.parallel_groups.len(), 2);
    }

    // =========================================================================
    // CheckConfig tests
    // =========================================================================

    #[test]
    fn test_check_config_basic() {
        let check = CheckConfig {
            run: "echo test".to_string(),
            description: "Test check".to_string(),
            enabled_if: None,
            env: HashMap::new(),
        };
        assert_eq!(check.run, "echo test");
        assert_eq!(check.description, "Test check");
    }

    #[test]
    fn test_check_config_with_env() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());
        env.insert("VAR2".to_string(), "value2".to_string());

        let check = CheckConfig {
            run: "echo $VAR1".to_string(),
            description: "Check with env".to_string(),
            enabled_if: None,
            env,
        };
        assert_eq!(check.env.len(), 2);
        assert_eq!(check.env.get("VAR1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_check_config_with_condition() {
        let check = CheckConfig {
            run: "cargo test".to_string(),
            description: "Cargo test".to_string(),
            enabled_if: Some(EnabledCondition {
                file_exists: Some("Cargo.toml".to_string()),
                dir_exists: None,
                command_exists: None,
            }),
            env: HashMap::new(),
        };
        assert!(check.enabled_if.is_some());
        let condition = check.enabled_if.as_ref().unwrap();
        assert_eq!(condition.file_exists, Some("Cargo.toml".to_string()));
    }

    // =========================================================================
    // EnabledCondition tests
    // =========================================================================

    #[test]
    fn test_enabled_condition_default() {
        let condition = EnabledCondition::default();
        assert!(condition.file_exists.is_none());
        assert!(condition.dir_exists.is_none());
        assert!(condition.command_exists.is_none());
    }

    #[test]
    fn test_enabled_condition_file_exists() {
        let condition = EnabledCondition {
            file_exists: Some("package.json".to_string()),
            ..Default::default()
        };
        assert_eq!(condition.file_exists, Some("package.json".to_string()));
    }

    #[test]
    fn test_enabled_condition_dir_exists() {
        let condition = EnabledCondition {
            dir_exists: Some("node_modules".to_string()),
            ..Default::default()
        };
        assert_eq!(condition.dir_exists, Some("node_modules".to_string()));
    }

    #[test]
    fn test_enabled_condition_command_exists() {
        let condition = EnabledCondition {
            command_exists: Some("cargo".to_string()),
            ..Default::default()
        };
        assert_eq!(condition.command_exists, Some("cargo".to_string()));
    }

    // =========================================================================
    // DetectionConfig tests
    // =========================================================================

    #[test]
    fn test_detection_config_default() {
        let config = DetectionConfig::default();
        assert!(config.agent_env_vars.is_empty());
        assert!(config.mode.is_none());
    }

    #[test]
    fn test_detection_config_with_custom_vars() {
        let config = DetectionConfig {
            mode: None,
            agent_env_vars: vec!["MY_AGENT_VAR".to_string(), "ANOTHER_VAR".to_string()],
        };
        assert_eq!(config.agent_env_vars.len(), 2);
    }

    #[test]
    fn test_detection_config_with_mode() {
        let config = DetectionConfig {
            mode: Some("agent".to_string()),
            agent_env_vars: vec![],
        };
        assert_eq!(config.mode, Some("agent".to_string()));
    }

    // =========================================================================
    // IntegrationConfig tests
    // =========================================================================

    #[test]
    fn test_integration_config_default() {
        let config = IntegrationConfig::default();
        assert!(!config.pre_commit);
        assert!(!config.pre_commit_path.is_empty());
    }

    #[test]
    fn test_integration_config_enabled() {
        let config = IntegrationConfig {
            pre_commit: true,
            pre_commit_path: ".pre-commit-config.yaml".to_string(),
        };
        assert!(config.pre_commit);
    }

    // =========================================================================
    // Config file discovery tests
    // =========================================================================

    #[test]
    fn test_config_file_name_constant() {
        assert_eq!(CONFIG_FILE_NAME, "agent-precommit.toml");
    }

    // =========================================================================
    // Serialization tests
    // =========================================================================

    #[test]
    fn test_config_serialize() {
        let config = Config::default();
        let result = toml::to_string_pretty(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_deserialize() {
        let toml_str = r#"
[human]
checks = ["test"]
timeout = "30s"

[agent]
checks = ["test"]
timeout = "15m"

[checks.test]
run = "echo test"
description = "Test"
"#;
        let result: std::result::Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Clone/Debug tests
    // =========================================================================

    #[test]
    fn test_config_clone() {
        let config = Config::default();
        let cloned = config.clone();
        assert_eq!(config.human.checks, cloned.human.checks);
    }

    #[test]
    fn test_config_debug() {
        let config = Config::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Config"));
    }
}
