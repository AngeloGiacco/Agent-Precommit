# Contributing to agent-precommit

Thank you for your interest in contributing to agent-precommit! This document provides guidelines for contributing.

## Development Setup

### Prerequisites

- Rust 1.75 or later
- Python 3.8+ (for Python wrapper development)
- Git

### Getting Started

1. Clone the repository:

   ```bash
   git clone https://github.com/agent-precommit/agent-precommit.git
   cd agent-precommit
   ```

2. Install pre-commit hooks:

   ```bash
   pip install pre-commit
   pre-commit install
   ```

3. Build the project:

   ```bash
   cargo build
   ```

4. Run tests:

   ```bash
   cargo test
   ```

## Code Quality Standards

This project maintains extremely high code quality standards:

### Rust

- **Formatting**: All code must pass `cargo fmt --check`
- **Linting**: All code must pass `cargo clippy` with no warnings
- **Tests**: All tests must pass
- **Documentation**: Public APIs must be documented

### Linting Configuration

The project uses strict Clippy linting. See `Cargo.toml` for the full configuration:

- `unsafe_code = "deny"` - No unsafe code allowed
- `unwrap_used = "deny"` - No `.unwrap()` calls
- `expect_used = "deny"` - No `.expect()` calls
- `panic = "deny"` - No explicit panics
- `todo = "deny"` - No TODO macros in production code

### Pre-commit Hooks

The following checks run automatically on each commit:

- `cargo fmt` - Code formatting
- `cargo clippy` - Lints
- `cargo check` - Compilation check
- Various file quality checks

## Making Changes

### Branching

- Create feature branches from `main`
- Use descriptive branch names: `feature/add-go-preset`, `fix/timeout-handling`

### Commits

- Write clear, concise commit messages
- Use conventional commit format:
  - `feat:` for new features
  - `fix:` for bug fixes
  - `docs:` for documentation
  - `refactor:` for code refactoring
  - `test:` for tests
  - `chore:` for maintenance

### Pull Requests

1. Ensure all CI checks pass
2. Update documentation if needed
3. Add tests for new functionality
4. Request review from maintainers

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run integration tests
cargo test --test '*'
```

### Writing Tests

- Place unit tests in the same file as the code
- Use the `#[cfg(test)]` attribute
- Place integration tests in `tests/`
- Aim for high coverage on critical paths

## Documentation

### Code Documentation

- Document all public functions, structs, and modules
- Use `///` for doc comments
- Include examples where helpful

### Building Docs

```bash
cargo doc --open
```

## Architecture Overview

```text
src/
├── lib.rs           # Library root
├── main.rs          # CLI entry point
├── cli/             # CLI commands
│   ├── mod.rs       # CLI structure
│   └── commands.rs  # Command implementations
├── config/          # Configuration handling
│   └── mod.rs       # Config structs and loading
├── core/            # Core functionality
│   ├── mod.rs       # Module root
│   ├── detector.rs  # Mode detection
│   ├── error.rs     # Error types
│   ├── executor.rs  # Command execution
│   ├── git.rs       # Git operations
│   └── runner.rs    # Check orchestration
├── checks/          # Check implementations
│   ├── mod.rs       # Module root
│   ├── builtin.rs   # Built-in checks
│   └── precommit.rs # Pre-commit integration
└── presets/         # Configuration presets
    └── mod.rs       # Preset definitions
```

## Release Process

1. Update version in `Cargo.toml` and `pyproject.toml`
2. Update CHANGELOG.md
3. Create a git tag: `git tag v0.x.0`
4. Push the tag: `git push origin v0.x.0`
5. GitHub Actions handles the rest

## Getting Help

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Join discussions in pull requests

## Code of Conduct

Be respectful and inclusive. We welcome contributions from everyone.
