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
pub fn run() -> Result<ExitCode> {
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
            commands::run(mode.as_deref(), check.as_deref(), all)
        },
        Some(Commands::Detect) => commands::detect(),
        Some(Commands::List { mode }) => commands::list(mode.as_deref()),
        Some(Commands::Validate) => commands::validate(),
        Some(Commands::Config { raw }) => commands::config(raw),
        Some(Commands::Completions { shell }) => {
            commands::completions(shell);
            Ok(ExitCode::SUCCESS)
        },
        None => commands::run(None, None, false),
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
    fn test_cli_parsing() {
        // Test that CLI parses without errors
        let cli = Cli::try_parse_from(["apc", "--help"]);
        // --help causes early exit, so this will be an error
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_version() {
        let cli = Cli::try_parse_from(["apc", "--version"]);
        assert!(cli.is_err()); // --version causes early exit
    }

    #[test]
    fn test_cli_subcommands() {
        let cli = Cli::try_parse_from(["apc", "init"]);
        assert!(cli.is_ok());

        let cli = Cli::try_parse_from(["apc", "run", "--mode", "human"]);
        assert!(cli.is_ok());

        let cli = Cli::try_parse_from(["apc", "detect"]);
        assert!(cli.is_ok());
    }
}
