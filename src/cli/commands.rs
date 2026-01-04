//! CLI command implementations.

use crate::config::{Config, CONFIG_FILE_NAME};
use crate::core::detector::{Detector, Mode};
use crate::core::error::{Error, Result};
use crate::core::git::GitRepo;
use crate::core::runner::Runner;
use console::style;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

/// Hook script template.
const HOOK_SCRIPT: &str = r#"#!/bin/sh
# agent-precommit hook - installed by `apc install`
# https://github.com/agent-precommit/agent-precommit

# Skip if APC_SKIP is set
if [ "$APC_SKIP" = "1" ]; then
    exit 0
fi

# Run agent-precommit
exec apc run
"#;

/// Hook marker comment.
const HOOK_MARKER: &str = "# agent-precommit hook";

/// Initialize configuration.
pub fn init(preset: Option<&str>, force: bool) -> Result<ExitCode> {
    let config_path = PathBuf::from(CONFIG_FILE_NAME);

    // Check if config already exists
    if config_path.exists() && !force {
        eprintln!(
            "{} Configuration already exists: {}",
            style("!").yellow(),
            config_path.display()
        );
        eprintln!("  Use --force to overwrite.");
        return Ok(ExitCode::FAILURE);
    }

    // Generate config
    let config = match preset {
        Some(p) => Config::for_preset(p),
        None => {
            // Auto-detect existing pre-commit config
            let mut config = Config::default();
            if PathBuf::from(".pre-commit-config.yaml").exists() {
                config.integration.pre_commit = true;
                eprintln!(
                    "{} Detected .pre-commit-config.yaml - enabling integration",
                    style("•").cyan()
                );
            }
            config
        },
    };

    // Write config
    let toml = toml::to_string_pretty(&config).map_err(|e| Error::Internal {
        message: format!("Failed to serialize config: {e}"),
    })?;

    std::fs::write(&config_path, toml).map_err(|e| Error::io("write config", e))?;

    eprintln!("{} Created {}", style("✓").green(), config_path.display());

    if let Some(p) = preset {
        eprintln!("  Using preset: {p}");
    }

    eprintln!("\nNext steps:");
    eprintln!("  1. Review and customize {CONFIG_FILE_NAME}");
    eprintln!("  2. Run: apc install");

    Ok(ExitCode::SUCCESS)
}

/// Install git hook.
pub fn install(force: bool) -> Result<ExitCode> {
    let repo = GitRepo::discover()?;
    let hooks_dir = repo.hooks_dir();
    let hook_path = hooks_dir.join("pre-commit");

    // Create hooks directory if needed
    if !hooks_dir.exists() {
        std::fs::create_dir_all(&hooks_dir).map_err(|e| Error::io("create hooks dir", e))?;
    }

    // Check for existing hook
    if hook_path.exists() {
        let content =
            std::fs::read_to_string(&hook_path).map_err(|e| Error::io("read existing hook", e))?;

        // Check if it's our hook
        if content.contains(HOOK_MARKER) {
            eprintln!(
                "{} Hook already installed at {}",
                style("✓").green(),
                hook_path.display()
            );
            return Ok(ExitCode::SUCCESS);
        }

        if !force {
            return Err(Error::HookExists { path: hook_path });
        }

        // Backup existing hook
        let backup_path = hooks_dir.join("pre-commit.bak");
        std::fs::rename(&hook_path, &backup_path).map_err(|e| Error::io("backup hook", e))?;
        eprintln!(
            "{} Backed up existing hook to {}",
            style("•").cyan(),
            backup_path.display()
        );
    }

    // Write hook
    std::fs::write(&hook_path, HOOK_SCRIPT).map_err(|e| Error::io("write hook", e))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .map_err(|e| Error::io("get hook metadata", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).map_err(|e| Error::io("set hook perms", e))?;
    }

    eprintln!(
        "{} Installed pre-commit hook at {}",
        style("✓").green(),
        hook_path.display()
    );

    Ok(ExitCode::SUCCESS)
}

/// Uninstall git hook.
pub fn uninstall() -> Result<ExitCode> {
    let repo = GitRepo::discover()?;
    let hook_path = repo.hook_path("pre-commit");

    if !hook_path.exists() {
        eprintln!(
            "{} No hook installed at {}",
            style("•").cyan(),
            hook_path.display()
        );
        return Ok(ExitCode::SUCCESS);
    }

    // Check if it's our hook
    let content = std::fs::read_to_string(&hook_path).map_err(|e| Error::io("read hook", e))?;

    if !content.contains(HOOK_MARKER) {
        eprintln!(
            "{} Hook at {} was not installed by agent-precommit",
            style("!").yellow(),
            hook_path.display()
        );
        eprintln!("  Remove manually if desired.");
        return Ok(ExitCode::FAILURE);
    }

    std::fs::remove_file(&hook_path).map_err(|e| Error::io("remove hook", e))?;

    eprintln!(
        "{} Removed pre-commit hook from {}",
        style("✓").green(),
        hook_path.display()
    );

    // Check for backup
    let backup_path = repo.hooks_dir().join("pre-commit.bak");
    if backup_path.exists() {
        eprintln!(
            "  Backup exists at {} - restore if needed",
            backup_path.display()
        );
    }

    Ok(ExitCode::SUCCESS)
}

/// Run checks.
pub fn run(mode_override: Option<&str>, check: Option<&str>, _all: bool) -> Result<ExitCode> {
    // Check for skip
    if std::env::var("APC_SKIP").ok().as_deref() == Some("1") {
        eprintln!("{} Skipping checks (APC_SKIP=1)", style("•").cyan());
        return Ok(ExitCode::SUCCESS);
    }

    // Load config
    let config = Config::load_or_default()?;

    // Detect or override mode
    let mode = if let Some(m) = mode_override {
        m.parse().map_err(|e: String| Error::ConfigInvalid {
            field: "mode".to_string(),
            message: e,
        })?
    } else {
        let detector = Detector::new(&config);
        let detection = detector.detect();
        eprintln!(
            "{} Mode: {} ({})",
            style("•").cyan(),
            style(detection.mode.name()).bold(),
            detection.reason
        );
        detection.mode
    };

    // Create runner
    let runner = Runner::new(config);

    // Run checks
    let result = tokio::runtime::Runtime::new()
        .map_err(|e| Error::Internal {
            message: format!("Failed to create runtime: {e}"),
        })?
        .block_on(async {
            if let Some(name) = check {
                let check_result = runner.run_single(name, mode).await?;
                Ok(crate::core::runner::RunResult {
                    mode,
                    checks: vec![check_result],
                    duration: std::time::Duration::ZERO,
                })
            } else {
                runner.run(mode).await
            }
        })?;

    // Print summary
    eprintln!();
    if result.success() {
        eprintln!(
            "{} All checks passed ({} passed, {} skipped) in {:?}",
            style("✓").green().bold(),
            result.passed_count(),
            result.skipped_count(),
            result.duration
        );
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!(
            "{} {} check(s) failed",
            style("✗").red().bold(),
            result.failed_count()
        );

        // Show failed check details
        for check in result.failed_checks() {
            eprintln!();
            eprintln!("  {} {}", style("Failed:").red(), check.name);
            if !check.output.combined_output().is_empty() {
                for line in check.output.combined_output().lines().take(20) {
                    eprintln!("    {line}");
                }
            }
        }

        Ok(ExitCode::FAILURE)
    }
}

/// Show detected mode.
pub fn detect() -> Result<ExitCode> {
    let config = Config::load_or_default()?;
    let detector = Detector::new(&config);
    let detection = detector.detect();

    eprintln!("Detected mode: {}", style(detection.mode.name()).bold());
    eprintln!("Reason: {}", detection.reason);

    // Show environment info
    eprintln!();
    eprintln!("Environment:");

    let env_vars = ["APC_MODE", "AGENT_MODE", "CI", "GITHUB_ACTIONS"];
    for var in env_vars {
        if let Ok(value) = std::env::var(var) {
            eprintln!("  {var}={value}");
        }
    }

    eprintln!();
    eprintln!(
        "TTY: stdin={}, stdout={}",
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal()
    );

    Ok(ExitCode::SUCCESS)
}

/// List configured checks.
pub fn list(mode: Option<&str>) -> Result<ExitCode> {
    let config = Config::load_or_default()?;

    let mode: Option<Mode> =
        mode.map(|m| m.parse())
            .transpose()
            .map_err(|e: String| Error::ConfigInvalid {
                field: "mode".to_string(),
                message: e,
            })?;

    // Print checks by mode
    if mode.is_none() || mode == Some(Mode::Human) {
        eprintln!("{}", style("Human mode checks:").bold());
        for name in &config.human.checks {
            print_check(&config, name);
        }
        eprintln!();
    }

    if mode.is_none() || mode == Some(Mode::Agent) || mode == Some(Mode::Ci) {
        eprintln!("{}", style("Agent mode checks:").bold());
        for name in &config.agent.checks {
            print_check(&config, name);
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Prints a check's details.
fn print_check(config: &Config, name: &str) {
    let check = config.checks.get(name);
    let description = check
        .map(|c| c.description.as_str())
        .filter(|d| !d.is_empty())
        .unwrap_or("(no description)");

    eprintln!("  {} - {}", style(name).cyan(), description);
}

/// Validate configuration.
pub fn validate() -> Result<ExitCode> {
    match Config::load() {
        Ok(config) => match config.validate() {
            Ok(()) => {
                eprintln!("{} Configuration is valid", style("✓").green());
                Ok(ExitCode::SUCCESS)
            },
            Err(e) => {
                eprintln!("{} Configuration validation failed: {e}", style("✗").red());
                Ok(ExitCode::FAILURE)
            },
        },
        Err(Error::ConfigNotFound { path }) => {
            eprintln!(
                "{} Configuration not found: {}",
                style("!").yellow(),
                path.display()
            );
            eprintln!("  Run: apc init");
            Ok(ExitCode::FAILURE)
        },
        Err(e) => {
            eprintln!("{} Failed to load configuration: {e}", style("✗").red());
            Ok(ExitCode::FAILURE)
        },
    }
}

/// Show configuration.
pub fn config(raw: bool) -> Result<ExitCode> {
    match Config::find_config_file() {
        Ok(path) => {
            eprintln!("Configuration file: {}", path.display());

            if raw {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| Error::io("read config", e))?;
                eprintln!();
                std::io::stdout()
                    .write_all(content.as_bytes())
                    .map_err(|e| Error::io("write output", e))?;
            }

            Ok(ExitCode::SUCCESS)
        },
        Err(Error::ConfigNotFound { .. }) => {
            eprintln!("{} No configuration file found", style("!").yellow());
            eprintln!("  Run: apc init");
            Ok(ExitCode::FAILURE)
        },
        Err(e) => Err(e),
    }
}

/// Generate shell completions.
pub fn completions(shell: clap_complete::Shell) {
    use clap::CommandFactory;
    clap_complete::generate(
        shell,
        &mut super::Cli::command(),
        "apc",
        &mut std::io::stdout(),
    );
}

use std::io::IsTerminal;
