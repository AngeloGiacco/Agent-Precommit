//! # agent-precommit
//!
//! Smart pre-commit hooks for humans and AI coding agents.
//!
//! Humans commit often and need fast checks. Agents commit once and need thorough checks.
//! `agent-precommit` auto-detects which is which and runs the right checks.
//!
//! ## Features
//!
//! - **Automatic mode detection**: Detects human vs agent commits via environment variables,
//!   TTY detection, and heuristics
//! - **Pre-commit integration**: Works with existing `.pre-commit-config.yaml` setups
//! - **Configurable checks**: Define custom checks for each mode in `agent-precommit.toml`
//! - **Parallel execution**: Run independent checks concurrently for faster agent-mode runs
//!
//! ## Example
//!
//! ```rust,no_run
//! use agent_precommit::{Config, Detector, Mode, Runner};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load configuration
//!     let config = Config::load_or_default()?;
//!
//!     // Detect mode (human, agent, or ci)
//!     let detector = Detector::new(&config);
//!     let mode = detector.detect();
//!
//!     // Run appropriate checks
//!     let runner = Runner::new(config);
//!     let result = runner.run(mode).await?;
//!
//!     if result.success() {
//!         Ok(())
//!     } else {
//!         std::process::exit(1);
//!     }
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/agent-precommit/0.1.0")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod checks;
pub mod cli;
pub mod config;
pub mod core;
pub mod presets;

// Re-export main types for convenience
pub use config::Config;
pub use core::detector::{Detector, Mode};
pub use core::error::{Error, Result};
pub use core::runner::{CheckResult, Runner, RunResult};
