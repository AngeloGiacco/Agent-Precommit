//! Configuration presets for common project types.
//!
//! Presets provide sensible default configurations for different tech stacks.

/// Available preset names.
pub mod names {
    /// Python projects (pytest, ruff, mypy).
    pub const PYTHON: &str = "python";
    /// Node.js/TypeScript projects (npm, eslint, jest).
    pub const NODE: &str = "node";
    /// Rust projects (cargo, clippy).
    pub const RUST: &str = "rust";
    /// Go projects (go test, golangci-lint).
    pub const GO: &str = "go";
}

/// Returns a list of available preset names.
#[must_use]
pub const fn available() -> &'static [&'static str] {
    &[names::PYTHON, names::NODE, names::RUST, names::GO]
}

/// Returns true if the preset name is valid.
#[must_use]
pub fn is_valid(name: &str) -> bool {
    available().contains(&name)
}

/// Returns a description for a preset.
#[must_use]
pub fn description(name: &str) -> &'static str {
    match name {
        names::PYTHON => "Python projects (pytest, ruff, mypy, pre-commit integration)",
        names::NODE => "Node.js/TypeScript projects (npm, eslint, jest, tsc)",
        names::RUST => "Rust projects (cargo fmt, clippy, cargo test)",
        names::GO => "Go projects (gofmt, golangci-lint, go test)",
        _ => "Unknown preset",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available() {
        assert!(!available().is_empty());
        assert!(available().contains(&"python"));
        assert!(available().contains(&"rust"));
    }

    #[test]
    fn test_is_valid() {
        assert!(is_valid("python"));
        assert!(is_valid("node"));
        assert!(!is_valid("invalid"));
    }

    #[test]
    fn test_description() {
        assert!(!description("python").is_empty());
        assert!(!description("rust").is_empty());
    }
}
