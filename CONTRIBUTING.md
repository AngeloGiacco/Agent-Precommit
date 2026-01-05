# Contributing to agent-precommit

We welcome contributions! Here's how to get started.

## Development Setup

```bash
# Clone and build
git clone https://github.com/agent-precommit/agent-precommit.git
cd agent-precommit
cargo build

# Install pre-commit hooks
pip install pre-commit
pre-commit install

# Run tests
cargo test
```

**Requirements:** Rust 1.75+, Python 3.8+ (optional, for wrapper)

## Making Changes

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make your changes
4. Run checks: `cargo fmt && cargo clippy && cargo test`
5. Commit using [conventional commits](https://www.conventionalcommits.org/):
   - `feat:` new features
   - `fix:` bug fixes
   - `docs:` documentation
   - `refactor:` code refactoring
6. Push and open a Pull Request

## Code Standards

- Format with `cargo fmt`
- Pass `cargo clippy --all-targets --all-features -- -D warnings`
- Add tests for new functionality
- No unsafe code, panics, or unwraps in production code

## Reporting Issues

Open an issue with:

- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Your environment (OS, Rust version)

## Questions?

Open a discussion or issue—we're happy to help!
