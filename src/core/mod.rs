//! Core functionality for agent-precommit.
//!
//! This module contains the main components:
//! - [`detector`]: Mode detection (human, agent, CI)
//! - [`runner`]: Check execution engine
//! - [`error`]: Error types and result handling
//! - [`git`]: Git repository operations

pub mod detector;
pub mod error;
pub mod executor;
pub mod git;
pub mod runner;
