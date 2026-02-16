//! Command-line interface for agent-precommit.
//!
//! This module provides the `apc` CLI with subcommands for:
//! - `init`: Initialize configuration
//! - `install`: Install git hook
//! - `uninstall`: Remove git hook
//! - `run`: Run checks manually
//! - `detect`: Show detected mode
//! - `list`: List configured checks
//! - `validate`: Validate configuration

mod commands;

use crate::core::error::Result;
use clap::{Parser, Subcommand};
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

/// Smart pre-commit hooks for humans and AI coding agents.
#[derive(Debug, Parser)]
#[command(
    name = "apc",
    author,
    version,
    about = "Smart pre-commit hooks for humans and AI coding agents",
    long_about = r#"
agent-precommit (apc) provides intelligent pre-commit hooks that detect
whether a commit is being made by a human or an AI coding agent.

Human commits get fast, staged-only checks.
Agent commits get thorough, merge-ready validation.

Quick start:
  apc init      # Create configuration
  apc install   # Install git hook
  # Done! Commits now auto-detect mode.

Environment variables:
  APC_MODE=human|agent|ci   Force a specific mode
  AGENT_MODE=1              Trigger agent mode
  APC_SKIP=1                Skip all checks
"#,
    propagate_version = true
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-error output.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Use color output.
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorChoice,
}

/// Color output choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ColorChoice {
    /// Always use color.
    Always,
    /// Auto-detect color support.
    #[default]
    Auto,
    /// Never use color.
    Never,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize agent-precommit configuration.
    #[command(visible_alias = "i")]
    Init {
        /// Use a preset configuration.
        #[arg(short, long, value_parser = ["python", "node", "rust", "go"])]
        preset: Option<String>,

        /// Overwrite existing configuration.
        #[arg(short, long)]
        force: bool,
    },

    /// Install the git pre-commit hook.
    Install {
        /// Overwrite existing hook.
        #[arg(short, long)]
        force: bool,
    },

    /// Remove the git pre-commit hook.
    Uninstall,

    /// Run checks manually.
    #[command(visible_alias = "r")]
    Run {
        /// Force a specific mode.
        #[arg(short, long, value_parser = ["human", "agent", "ci"])]
        mode: Option<String>,

        /// Run only a specific check.
        #[arg(short, long)]
        check: Option<String>,

        /// Run all checks regardless of conditions.
        #[arg(long)]
        all: bool,
    },

    /// Show the detected mode and reasoning.
    #[command(visible_alias = "d")]
    Detect,

    /// List all configured checks.
    #[command(visible_alias = "l")]
    List {
        /// Show checks for a specific mode.
        #[arg(short, long, value_parser = ["human", "agent", "ci"])]
        mode: Option<String>,
    },

    /// Validate the configuration file.
    #[command(visible_alias = "v")]
    Validate,

    /// Show configuration file location and contents.
    Config {
        /// Output raw TOML.
        #[arg(long)]
        raw: bool,
    },

    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// Runs the CLI.
pub async fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    // Set up logging
    setup_logging(cli.verbose, cli.quiet);

    // Set up color
    setup_color(cli.color);

    // If no subcommand, run the default action (same as `apc run`)
    match cli.command {
        Some(Commands::Init { preset, force }) => commands::init(preset.as_deref(), force),
        Some(Commands::Install { force }) => commands::install(force),
        Some(Commands::Uninstall) => commands::uninstall(),
        Some(Commands::Run { mode, check, all }) => {
            commands::run(mode.as_deref(), check.as_deref(), all).await
        },
        Some(Commands::Detect) => commands::detect(),
        Some(Commands::List { mode }) => commands::list(mode.as_deref()),
        Some(Commands::Validate) => commands::validate(),
        Some(Commands::Config { raw }) => commands::config(raw),
        Some(Commands::Completions { shell }) => {
            commands::completions(shell);
            Ok(ExitCode::SUCCESS)
        },
        None => commands::run(None, None, false).await,
    }
}

/// Sets up logging based on verbosity flags.
fn setup_logging(verbose: bool, quiet: bool) {
    let filter = if quiet {
        "error"
    } else if verbose {
        "debug"
    } else {
        "info"
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

/// Sets up color output.
fn setup_color(choice: ColorChoice) {
    match choice {
        ColorChoice::Always => {
            console::set_colors_enabled(true);
            console::set_colors_enabled_stderr(true);
        },
        ColorChoice::Never => {
            console::set_colors_enabled(false);
            console::set_colors_enabled_stderr(false);
        },
        ColorChoice::Auto => {
            // Let console crate auto-detect
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_help() {
        let cli = Cli::try_parse_from(["apc", "--help"]);
        // --help causes early exit, so this will be an error
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_version() {
        let cli = Cli::try_parse_from(["apc", "--version"]);
        assert!(cli.is_err()); // --version causes early exit
    }

    // =========================================================================
    // Subcommand parsing tests
    // =========================================================================

    #[test]
    fn test_parse_init() {
        let cli = Cli::try_parse_from(["apc", "init"]).expect("parse init");
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                preset: None,
                force: false
            })
        ));
    }

    #[test]
    fn test_parse_init_with_preset() {
        let cli = Cli::try_parse_from(["apc", "init", "--preset", "rust"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                preset: Some(_),
                force: false
            })
        ));
    }

    #[test]
    fn test_parse_init_with_force() {
        let cli = Cli::try_parse_from(["apc", "init", "--force"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                preset: None,
                force: true
            })
        ));
    }

    #[test]
    fn test_parse_init_with_preset_and_force() {
        let cli =
            Cli::try_parse_from(["apc", "init", "--preset", "python", "--force"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                preset: Some(_),
                force: true
            })
        ));
    }

    #[test]
    fn test_parse_init_invalid_preset() {
        let result = Cli::try_parse_from(["apc", "init", "--preset", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_init_alias() {
        let cli = Cli::try_parse_from(["apc", "i"]).expect("parse init alias");
        assert!(matches!(cli.command, Some(Commands::Init { .. })));
    }

    #[test]
    fn test_parse_install() {
        let cli = Cli::try_parse_from(["apc", "install"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Install { force: false })
        ));
    }

    #[test]
    fn test_parse_install_with_force() {
        let cli = Cli::try_parse_from(["apc", "install", "--force"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Install { force: true })
        ));
    }

    #[test]
    fn test_parse_uninstall() {
        let cli = Cli::try_parse_from(["apc", "uninstall"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Uninstall)));
    }

    #[test]
    fn test_parse_run() {
        let cli = Cli::try_parse_from(["apc", "run"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Run {
                mode: None,
                check: None,
                all: false
            })
        ));
    }

    #[test]
    fn test_parse_run_with_mode() {
        let cli = Cli::try_parse_from(["apc", "run", "--mode", "human"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Run { mode: Some(_), .. })
        ));
    }

    #[test]
    fn test_parse_run_with_agent_mode() {
        let cli = Cli::try_parse_from(["apc", "run", "--mode", "agent"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Run { mode: Some(_), .. })
        ));
    }

    #[test]
    fn test_parse_run_with_ci_mode() {
        let cli = Cli::try_parse_from(["apc", "run", "--mode", "ci"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Run { mode: Some(_), .. })
        ));
    }

    #[test]
    fn test_parse_run_invalid_mode() {
        let result = Cli::try_parse_from(["apc", "run", "--mode", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_with_check() {
        let cli = Cli::try_parse_from(["apc", "run", "--check", "lint"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Run { check: Some(_), .. })
        ));
    }

    #[test]
    fn test_parse_run_with_all() {
        let cli = Cli::try_parse_from(["apc", "run", "--all"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Run { all: true, .. })));
    }

    #[test]
    fn test_parse_run_alias() {
        let cli = Cli::try_parse_from(["apc", "r"]).expect("parse run alias");
        assert!(matches!(cli.command, Some(Commands::Run { .. })));
    }

    #[test]
    fn test_parse_detect() {
        let cli = Cli::try_parse_from(["apc", "detect"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Detect)));
    }

    #[test]
    fn test_parse_detect_alias() {
        let cli = Cli::try_parse_from(["apc", "d"]).expect("parse detect alias");
        assert!(matches!(cli.command, Some(Commands::Detect)));
    }

    #[test]
    fn test_parse_list() {
        let cli = Cli::try_parse_from(["apc", "list"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::List { mode: None })));
    }

    #[test]
    fn test_parse_list_with_mode() {
        let cli = Cli::try_parse_from(["apc", "list", "--mode", "human"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::List { mode: Some(_) })
        ));
    }

    #[test]
    fn test_parse_list_alias() {
        let cli = Cli::try_parse_from(["apc", "l"]).expect("parse list alias");
        assert!(matches!(cli.command, Some(Commands::List { .. })));
    }

    #[test]
    fn test_parse_validate() {
        let cli = Cli::try_parse_from(["apc", "validate"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Validate)));
    }

    #[test]
    fn test_parse_validate_alias() {
        let cli = Cli::try_parse_from(["apc", "v"]).expect("parse validate alias");
        assert!(matches!(cli.command, Some(Commands::Validate)));
    }

    #[test]
    fn test_parse_config() {
        let cli = Cli::try_parse_from(["apc", "config"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Config { raw: false })));
    }

    #[test]
    fn test_parse_config_raw() {
        let cli = Cli::try_parse_from(["apc", "config", "--raw"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Config { raw: true })));
    }

    #[test]
    fn test_parse_completions_bash() {
        let cli = Cli::try_parse_from(["apc", "completions", "bash"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Completions { .. })));
    }

    #[test]
    fn test_parse_completions_zsh() {
        let cli = Cli::try_parse_from(["apc", "completions", "zsh"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Completions { .. })));
    }

    #[test]
    fn test_parse_completions_fish() {
        let cli = Cli::try_parse_from(["apc", "completions", "fish"]).expect("parse");
        assert!(matches!(cli.command, Some(Commands::Completions { .. })));
    }

    // =========================================================================
    // Global flags tests
    // =========================================================================

    #[test]
    fn test_parse_verbose_flag() {
        let cli = Cli::try_parse_from(["apc", "--verbose", "detect"]).expect("parse");
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_parse_quiet_flag() {
        let cli = Cli::try_parse_from(["apc", "--quiet", "detect"]).expect("parse");
        assert!(!cli.verbose);
        assert!(cli.quiet);
    }

    #[test]
    fn test_parse_color_always() {
        let cli = Cli::try_parse_from(["apc", "--color", "always", "detect"]).expect("parse");
        assert_eq!(cli.color, ColorChoice::Always);
    }

    #[test]
    fn test_parse_color_never() {
        let cli = Cli::try_parse_from(["apc", "--color", "never", "detect"]).expect("parse");
        assert_eq!(cli.color, ColorChoice::Never);
    }

    #[test]
    fn test_parse_color_auto_default() {
        let cli = Cli::try_parse_from(["apc", "detect"]).expect("parse");
        assert_eq!(cli.color, ColorChoice::Auto);
    }

    #[test]
    fn test_parse_no_subcommand() {
        let cli = Cli::try_parse_from(["apc"]).expect("parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_short_verbose() {
        let cli = Cli::try_parse_from(["apc", "-v", "detect"]).expect("parse");
        assert!(cli.verbose);
    }

    #[test]
    fn test_parse_short_quiet() {
        let cli = Cli::try_parse_from(["apc", "-q", "detect"]).expect("parse");
        assert!(cli.quiet);
    }

    // =========================================================================
    // ColorChoice tests
    // =========================================================================

    #[test]
    fn test_color_choice_default() {
        assert_eq!(ColorChoice::default(), ColorChoice::Auto);
    }

    #[test]
    fn test_color_choice_debug() {
        let debug_str = format!("{:?}", ColorChoice::Always);
        assert_eq!(debug_str, "Always");
    }

    #[test]
    fn test_color_choice_eq() {
        assert_eq!(ColorChoice::Always, ColorChoice::Always);
        assert_ne!(ColorChoice::Always, ColorChoice::Never);
    }

    // =========================================================================
    // Preset validation tests
    // =========================================================================

    #[test]
    fn test_all_valid_presets_accepted() {
        for preset in ["python", "node", "rust", "go"] {
            let result = Cli::try_parse_from(["apc", "init", "--preset", preset]);
            assert!(result.is_ok(), "Preset '{}' should be accepted", preset);
        }
    }

    #[test]
    fn test_all_valid_modes_accepted() {
        for mode in ["human", "agent", "ci"] {
            let result = Cli::try_parse_from(["apc", "run", "--mode", mode]);
            assert!(result.is_ok(), "Mode '{}' should be accepted", mode);
        }
    }
}
