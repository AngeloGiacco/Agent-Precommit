//! Mode detection for distinguishing human vs agent commits.
//!
//! The detector analyzes the environment to determine whether a commit
//! is being made by a human developer or an AI coding agent.

use crate::config::Config;
use std::env;
use std::io::IsTerminal;

/// The detected commit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// Human developer - fast checks, staged files only.
    #[default]
    Human,
    /// AI coding agent - thorough checks, full codebase.
    Agent,
    /// CI environment - same as agent, possibly with extra reporting.
    Ci,
}

impl Mode {
    /// Returns a human-readable name for the mode.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Ci => "ci",
        }
    }

    /// Returns whether this mode requires thorough checks.
    #[must_use]
    pub const fn is_thorough(&self) -> bool {
        matches!(self, Self::Agent | Self::Ci)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "ci" => Ok(Self::Ci),
            _ => Err(format!("Invalid mode: {s}. Expected: human, agent, or ci")),
        }
    }
}

/// Reason for mode detection - useful for debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionReason {
    /// Mode set via APC_MODE environment variable.
    ExplicitApcMode(String),
    /// Mode set via AGENT_MODE environment variable.
    ExplicitAgentMode,
    /// Known agent environment variable detected.
    KnownAgentEnvVar(String),
    /// Custom agent environment variable from config.
    CustomAgentEnvVar(String),
    /// CI environment detected.
    CiEnvironment(String),
    /// No TTY detected (non-interactive).
    NoTty,
    /// Default fallback to human mode.
    Default,
}

impl std::fmt::Display for DetectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitApcMode(value) => write!(f, "APC_MODE={value}"),
            Self::ExplicitAgentMode => write!(f, "AGENT_MODE=1"),
            Self::KnownAgentEnvVar(var) => write!(f, "Known agent env var: {var}"),
            Self::CustomAgentEnvVar(var) => write!(f, "Custom agent env var: {var}"),
            Self::CiEnvironment(var) => write!(f, "CI environment: {var}"),
            Self::NoTty => write!(f, "No TTY detected (non-interactive)"),
            Self::Default => write!(f, "Default (no agent indicators)"),
        }
    }
}

/// Result of mode detection.
#[derive(Debug, Clone)]
pub struct Detection {
    /// The detected mode.
    pub mode: Mode,
    /// Reason for the detection.
    pub reason: DetectionReason,
}

/// Detector for determining commit mode.
#[derive(Debug)]
pub struct Detector<'a> {
    config: &'a Config,
}

/// Known environment variables that indicate an AI agent.
const KNOWN_AGENT_ENV_VARS: &[&str] = &[
    // Claude Code
    "CLAUDE_CODE",
    "ANTHROPIC_PROJECT_ID",
    // Cursor
    "CURSOR_SESSION",
    "CURSOR_TRACE_ID",
    // Aider
    "AIDER_MODEL",
    "AIDER_CHAT_HISTORY_FILE",
    // OpenAI Codex / ChatGPT
    "CODEX_SESSION",
    "OPENAI_API_KEY_FOR_AGENT",
    // Devin
    "DEVIN_SESSION",
    "DEVIN_API_KEY",
    // Cline
    "CLINE_SESSION",
    "CLINE_API_KEY",
    // Continue.dev
    "CONTINUE_SESSION",
    "CONTINUE_GLOBAL_DIR",
    // GitHub Copilot Workspace
    "GITHUB_COPILOT_WORKSPACE",
    // Amazon CodeWhisperer / Q
    "AWS_CODEWHISPERER_SESSION",
    "AMAZON_Q_SESSION",
    // Sourcegraph Cody
    "CODY_SESSION",
    "SRC_ACCESS_TOKEN",
    // Tabnine
    "TABNINE_SESSION",
    // Replit Agent
    "REPLIT_AGENT",
    "REPL_ID",
    // Generic
    "AI_AGENT",
    "CODING_AGENT",
];

/// Known environment variables that indicate a CI environment.
const KNOWN_CI_ENV_VARS: &[&str] = &[
    "CI",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "CIRCLECI",
    "TRAVIS",
    "JENKINS_URL",
    "BUILDKITE",
    "BITBUCKET_PIPELINE",
    "AZURE_PIPELINES",
    "TEAMCITY_VERSION",
    "DRONE",
    "WOODPECKER",
    "SEMAPHORE",
    "APPVEYOR",
    "CODEBUILD_BUILD_ID",
    "TF_BUILD",
    "NETLIFY",
    "VERCEL",
    "RENDER",
    "RAILWAY_ENVIRONMENT",
    "FLY_APP_NAME",
];

impl<'a> Detector<'a> {
    /// Creates a new detector with the given configuration.
    #[must_use]
    pub const fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Detects the commit mode based on environment.
    #[must_use]
    pub fn detect(&self) -> Detection {
        // Priority 1: Explicit APC_MODE override
        if let Some(detection) = self.check_apc_mode() {
            return detection;
        }

        // Priority 2: AGENT_MODE=1 flag
        if let Some(detection) = self.check_agent_mode_flag() {
            return detection;
        }

        // Priority 3: Known agent environment variables
        if let Some(detection) = self.check_known_agent_env_vars() {
            return detection;
        }

        // Priority 4: Custom agent environment variables from config
        if let Some(detection) = self.check_custom_agent_env_vars() {
            return detection;
        }

        // Priority 5: CI environment detection
        if let Some(detection) = self.check_ci_environment() {
            return detection;
        }

        // Priority 6: TTY detection (fallback heuristic)
        if let Some(detection) = self.check_tty() {
            return detection;
        }

        // Default: Human mode
        Detection {
            mode: Mode::Human,
            reason: DetectionReason::Default,
        }
    }

    /// Checks for explicit APC_MODE environment variable.
    fn check_apc_mode(&self) -> Option<Detection> {
        env::var("APC_MODE").ok().map(|value| {
            let mode = value.parse().unwrap_or(Mode::Human);
            Detection {
                mode,
                reason: DetectionReason::ExplicitApcMode(value),
            }
        })
    }

    /// Checks for AGENT_MODE=1 flag.
    fn check_agent_mode_flag(&self) -> Option<Detection> {
        env::var("AGENT_MODE").ok().and_then(|value| {
            if value == "1" || value.eq_ignore_ascii_case("true") {
                Some(Detection {
                    mode: Mode::Agent,
                    reason: DetectionReason::ExplicitAgentMode,
                })
            } else {
                None
            }
        })
    }

    /// Checks for known agent environment variables.
    fn check_known_agent_env_vars(&self) -> Option<Detection> {
        for var in KNOWN_AGENT_ENV_VARS {
            if env::var(var).is_ok() {
                return Some(Detection {
                    mode: Mode::Agent,
                    reason: DetectionReason::KnownAgentEnvVar((*var).to_string()),
                });
            }
        }
        None
    }

    /// Checks for custom agent environment variables from config.
    fn check_custom_agent_env_vars(&self) -> Option<Detection> {
        for var in &self.config.detection.agent_env_vars {
            if env::var(var).is_ok() {
                return Some(Detection {
                    mode: Mode::Agent,
                    reason: DetectionReason::CustomAgentEnvVar(var.clone()),
                });
            }
        }
        None
    }

    /// Checks for CI environment variables.
    fn check_ci_environment(&self) -> Option<Detection> {
        for var in KNOWN_CI_ENV_VARS {
            if env::var(var).is_ok() {
                return Some(Detection {
                    mode: Mode::Ci,
                    reason: DetectionReason::CiEnvironment((*var).to_string()),
                });
            }
        }
        None
    }

    /// Checks for TTY presence (non-interactive = likely agent).
    fn check_tty(&self) -> Option<Detection> {
        let stdin_is_tty = std::io::stdin().is_terminal();
        let stdout_is_tty = std::io::stdout().is_terminal();

        // Only trigger if BOTH stdin and stdout are not TTY
        // This avoids false positives from piped commands
        if !stdin_is_tty && !stdout_is_tty {
            return Some(Detection {
                mode: Mode::Agent,
                reason: DetectionReason::NoTty,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Mode tests
    // =========================================================================

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Human.to_string(), "human");
        assert_eq!(Mode::Agent.to_string(), "agent");
        assert_eq!(Mode::Ci.to_string(), "ci");
    }

    #[test]
    fn test_mode_name() {
        assert_eq!(Mode::Human.name(), "human");
        assert_eq!(Mode::Agent.name(), "agent");
        assert_eq!(Mode::Ci.name(), "ci");
    }

    #[test]
    fn test_mode_parse() {
        assert_eq!("human".parse::<Mode>().ok(), Some(Mode::Human));
        assert_eq!("AGENT".parse::<Mode>().ok(), Some(Mode::Agent));
        assert_eq!("CI".parse::<Mode>().ok(), Some(Mode::Ci));
        assert!("invalid".parse::<Mode>().is_err());
    }

    #[test]
    fn test_mode_parse_case_insensitive() {
        assert_eq!("Human".parse::<Mode>().ok(), Some(Mode::Human));
        assert_eq!("HUMAN".parse::<Mode>().ok(), Some(Mode::Human));
        assert_eq!("agent".parse::<Mode>().ok(), Some(Mode::Agent));
        assert_eq!("Agent".parse::<Mode>().ok(), Some(Mode::Agent));
        assert_eq!("ci".parse::<Mode>().ok(), Some(Mode::Ci));
        assert_eq!("Ci".parse::<Mode>().ok(), Some(Mode::Ci));
    }

    #[test]
    fn test_mode_parse_error_message() {
        let err = "invalid"
            .parse::<Mode>()
            .expect_err("should fail to parse invalid");
        assert!(err.contains("Invalid mode"));
        assert!(err.contains("human, agent, or ci"));
    }

    #[test]
    fn test_mode_is_thorough() {
        assert!(!Mode::Human.is_thorough());
        assert!(Mode::Agent.is_thorough());
        assert!(Mode::Ci.is_thorough());
    }

    #[test]
    fn test_mode_default() {
        assert_eq!(Mode::default(), Mode::Human);
    }

    #[test]
    fn test_mode_clone() {
        let mode = Mode::Agent;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_mode_eq() {
        assert_eq!(Mode::Human, Mode::Human);
        assert_eq!(Mode::Agent, Mode::Agent);
        assert_eq!(Mode::Ci, Mode::Ci);
        assert_ne!(Mode::Human, Mode::Agent);
        assert_ne!(Mode::Agent, Mode::Ci);
        assert_ne!(Mode::Human, Mode::Ci);
    }

    #[test]
    fn test_mode_debug() {
        let debug_str = format!("{:?}", Mode::Human);
        assert_eq!(debug_str, "Human");
    }

    // =========================================================================
    // DetectionReason tests
    // =========================================================================

    #[test]
    fn test_detection_reason_display_explicit_apc_mode() {
        let reason = DetectionReason::ExplicitApcMode("agent".to_string());
        assert_eq!(reason.to_string(), "APC_MODE=agent");
    }

    #[test]
    fn test_detection_reason_display_explicit_agent_mode() {
        let reason = DetectionReason::ExplicitAgentMode;
        assert_eq!(reason.to_string(), "AGENT_MODE=1");
    }

    #[test]
    fn test_detection_reason_display_known_agent_env_var() {
        let reason = DetectionReason::KnownAgentEnvVar("CLAUDE_CODE".to_string());
        assert_eq!(reason.to_string(), "Known agent env var: CLAUDE_CODE");
    }

    #[test]
    fn test_detection_reason_display_custom_agent_env_var() {
        let reason = DetectionReason::CustomAgentEnvVar("MY_AGENT_VAR".to_string());
        assert_eq!(reason.to_string(), "Custom agent env var: MY_AGENT_VAR");
    }

    #[test]
    fn test_detection_reason_display_ci_environment() {
        let reason = DetectionReason::CiEnvironment("GITHUB_ACTIONS".to_string());
        assert_eq!(reason.to_string(), "CI environment: GITHUB_ACTIONS");
    }

    #[test]
    fn test_detection_reason_display_no_tty() {
        let reason = DetectionReason::NoTty;
        assert_eq!(reason.to_string(), "No TTY detected (non-interactive)");
    }

    #[test]
    fn test_detection_reason_display_default() {
        let reason = DetectionReason::Default;
        assert_eq!(reason.to_string(), "Default (no agent indicators)");
    }

    #[test]
    fn test_detection_reason_equality() {
        assert_eq!(
            DetectionReason::ExplicitAgentMode,
            DetectionReason::ExplicitAgentMode
        );
        assert_eq!(DetectionReason::NoTty, DetectionReason::NoTty);
        assert_eq!(DetectionReason::Default, DetectionReason::Default);
        assert_eq!(
            DetectionReason::ExplicitApcMode("agent".to_string()),
            DetectionReason::ExplicitApcMode("agent".to_string())
        );
        assert_ne!(
            DetectionReason::ExplicitApcMode("agent".to_string()),
            DetectionReason::ExplicitApcMode("human".to_string())
        );
    }

    #[test]
    fn test_detection_reason_clone() {
        let reason = DetectionReason::KnownAgentEnvVar("CLAUDE_CODE".to_string());
        let cloned = reason.clone();
        assert_eq!(reason, cloned);
    }

    // =========================================================================
    // Detection tests
    // =========================================================================

    #[test]
    fn test_detection_struct() {
        let detection = Detection {
            mode: Mode::Agent,
            reason: DetectionReason::ExplicitAgentMode,
        };
        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(detection.reason, DetectionReason::ExplicitAgentMode);
    }

    #[test]
    fn test_detection_clone() {
        let detection = Detection {
            mode: Mode::Ci,
            reason: DetectionReason::CiEnvironment("CI".to_string()),
        };
        let cloned = detection.clone();
        assert_eq!(detection.mode, cloned.mode);
        assert_eq!(detection.reason, cloned.reason);
    }

    // =========================================================================
    // Detector tests
    // =========================================================================

    #[test]
    fn test_detector_new() {
        let config = Config::default();
        let detector = Detector::new(&config);
        // Just verify we can create a detector
        let debug_str = format!("{:?}", detector);
        assert!(debug_str.contains("Detector"));
    }

    // =========================================================================
    // Known env var list tests
    // =========================================================================

    #[test]
    fn test_known_agent_env_vars_has_expected_count() {
        // Verify we have a reasonable number of known agent env vars
        assert!(KNOWN_AGENT_ENV_VARS.len() >= 10);
    }

    #[test]
    fn test_known_agent_env_vars_contains_claude_code() {
        assert!(KNOWN_AGENT_ENV_VARS.contains(&"CLAUDE_CODE"));
    }

    #[test]
    fn test_known_agent_env_vars_contains_cursor() {
        assert!(KNOWN_AGENT_ENV_VARS.contains(&"CURSOR_SESSION"));
    }

    #[test]
    fn test_known_agent_env_vars_contains_aider() {
        assert!(KNOWN_AGENT_ENV_VARS.contains(&"AIDER_MODEL"));
    }

    #[test]
    fn test_known_ci_env_vars_has_expected_count() {
        // Verify we have a reasonable number of known CI env vars
        assert!(KNOWN_CI_ENV_VARS.len() >= 10);
    }

    #[test]
    fn test_known_ci_env_vars_contains_ci() {
        assert!(KNOWN_CI_ENV_VARS.contains(&"CI"));
    }

    #[test]
    fn test_known_ci_env_vars_contains_github_actions() {
        assert!(KNOWN_CI_ENV_VARS.contains(&"GITHUB_ACTIONS"));
    }

    #[test]
    fn test_known_ci_env_vars_contains_gitlab_ci() {
        assert!(KNOWN_CI_ENV_VARS.contains(&"GITLAB_CI"));
    }

    // =========================================================================
    // Detector.detect() tests with env var control
    //
    // These tests modify process-global env vars, so they must run with
    // --test-threads=1. They are ignored in default parallel test runs.
    // Run with: cargo test -- --ignored --test-threads=1
    // =========================================================================

    /// Env var manipulation helpers for tests.
    ///
    /// These tests are `#[ignore]`d by default and must be run with
    /// `--test-threads=1` to avoid data races on process env vars.
    #[allow(deprecated, unsafe_code)]
    mod env_helpers {
        use std::env;

        pub struct EnvGuard {
            vars: Vec<(String, Option<String>)>,
        }

        impl EnvGuard {
            pub fn new() -> Self {
                Self { vars: Vec::new() }
            }

            pub fn set(&mut self, key: &str, value: &str) {
                let prev = env::var(key).ok();
                self.vars.push((key.to_string(), prev));
                // SAFETY: These tests run single-threaded via --test-threads=1
                unsafe { env::set_var(key, value) };
            }

            pub fn remove(&mut self, key: &str) {
                let prev = env::var(key).ok();
                self.vars.push((key.to_string(), prev));
                // SAFETY: These tests run single-threaded via --test-threads=1
                unsafe { env::remove_var(key) };
            }

            /// Remove all known detection env vars to get a clean state.
            pub fn clear_all_detection_vars(&mut self) {
                self.remove("APC_MODE");
                self.remove("AGENT_MODE");
                for var in super::super::KNOWN_AGENT_ENV_VARS {
                    self.remove(var);
                }
                for var in super::super::KNOWN_CI_ENV_VARS {
                    self.remove(var);
                }
            }
        }

        impl Drop for EnvGuard {
            fn drop(&mut self) {
                for (key, value) in self.vars.iter().rev() {
                    match value {
                        // SAFETY: These tests run single-threaded via --test-threads=1
                        Some(v) => unsafe { env::set_var(key, v) },
                        None => unsafe { env::remove_var(key) },
                    }
                }
            }
        }
    }

    use env_helpers::EnvGuard;

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_apc_mode_human() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("APC_MODE", "human");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Human);
        assert!(matches!(
            detection.reason,
            DetectionReason::ExplicitApcMode(_)
        ));
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_apc_mode_agent() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("APC_MODE", "agent");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert!(matches!(
            detection.reason,
            DetectionReason::ExplicitApcMode(_)
        ));
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_apc_mode_ci() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("APC_MODE", "ci");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Ci);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_apc_mode_invalid_falls_back_to_human() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("APC_MODE", "invalid_value");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        // Invalid APC_MODE parses to Human (the unwrap_or default)
        assert_eq!(detection.mode, Mode::Human);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_agent_mode_flag() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("AGENT_MODE", "1");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(detection.reason, DetectionReason::ExplicitAgentMode);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_agent_mode_flag_true() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("AGENT_MODE", "true");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(detection.reason, DetectionReason::ExplicitAgentMode);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_agent_mode_flag_false_ignored() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("AGENT_MODE", "0");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        // AGENT_MODE=0 should NOT trigger agent mode
        assert_ne!(detection.reason, DetectionReason::ExplicitAgentMode);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_known_agent_env_var_claude_code() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("CLAUDE_CODE", "1");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(
            detection.reason,
            DetectionReason::KnownAgentEnvVar("CLAUDE_CODE".to_string())
        );
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_known_agent_env_var_cursor() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("CURSOR_SESSION", "test-session");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(
            detection.reason,
            DetectionReason::KnownAgentEnvVar("CURSOR_SESSION".to_string())
        );
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_custom_agent_env_var() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("MY_CUSTOM_AGENT_VAR_12345", "1");

        let mut config = Config::default();
        config.detection.agent_env_vars = vec!["MY_CUSTOM_AGENT_VAR_12345".to_string()];

        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(
            detection.reason,
            DetectionReason::CustomAgentEnvVar("MY_CUSTOM_AGENT_VAR_12345".to_string())
        );

        // Clean up via the guard's drop
        guard.remove("MY_CUSTOM_AGENT_VAR_12345");
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_ci_environment() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("GITHUB_ACTIONS", "true");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        assert_eq!(detection.mode, Mode::Ci);
        assert_eq!(
            detection.reason,
            DetectionReason::CiEnvironment("GITHUB_ACTIONS".to_string())
        );
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_priority_apc_mode_over_agent_mode() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("APC_MODE", "human");
        guard.set("AGENT_MODE", "1");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        // APC_MODE should take priority over AGENT_MODE
        assert_eq!(detection.mode, Mode::Human);
        assert!(matches!(
            detection.reason,
            DetectionReason::ExplicitApcMode(_)
        ));
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_priority_agent_mode_over_known_vars() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("AGENT_MODE", "1");
        guard.set("CI", "true");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        // AGENT_MODE should take priority over CI
        assert_eq!(detection.mode, Mode::Agent);
        assert_eq!(detection.reason, DetectionReason::ExplicitAgentMode);
    }

    #[test]
    #[ignore = "modifies global env vars, must run with --test-threads=1"]
    fn test_detect_priority_known_vars_over_ci() {
        let mut guard = EnvGuard::new();
        guard.clear_all_detection_vars();
        guard.set("CLAUDE_CODE", "1");
        guard.set("CI", "true");

        let config = Config::default();
        let detector = Detector::new(&config);
        let detection = detector.detect();

        // Known agent vars should take priority over CI
        assert_eq!(detection.mode, Mode::Agent);
        assert!(matches!(
            detection.reason,
            DetectionReason::KnownAgentEnvVar(_)
        ));
    }

    #[test]
    fn test_known_agent_env_vars_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for var in KNOWN_AGENT_ENV_VARS {
            assert!(seen.insert(var), "Duplicate agent env var: {}", var);
        }
    }

    #[test]
    fn test_known_ci_env_vars_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for var in KNOWN_CI_ENV_VARS {
            assert!(seen.insert(var), "Duplicate CI env var: {}", var);
        }
    }

    #[test]
    fn test_known_agent_and_ci_vars_no_overlap() {
        for agent_var in KNOWN_AGENT_ENV_VARS {
            assert!(
                !KNOWN_CI_ENV_VARS.contains(agent_var),
                "Env var {} appears in both agent and CI lists",
                agent_var
            );
        }
    }

    #[test]
    fn test_mode_hash() {
        let mut set = std::collections::HashSet::new();
        set.insert(Mode::Human);
        set.insert(Mode::Agent);
        set.insert(Mode::Ci);
        assert_eq!(set.len(), 3);
        set.insert(Mode::Human);
        assert_eq!(set.len(), 3);
    }
}
