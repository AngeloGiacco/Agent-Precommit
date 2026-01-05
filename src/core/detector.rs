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
        let err = "invalid".parse::<Mode>().expect_err("should fail to parse invalid");
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
}
