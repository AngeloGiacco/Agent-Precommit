# Makefile for agent-precommit development
# Usage: make <target>

.PHONY: all build release test lint fmt check clean install docs bench help

# Default target
all: fmt lint test build

# Build debug version
build:
	cargo build

# Build release version
release:
	cargo build --release

# Run all tests
test:
	cargo test --all-features

# Run tests with output
test-verbose:
	cargo test --all-features -- --nocapture

# Run integration tests only
test-integration:
	cargo test --test '*'

# Run lints (clippy)
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Format code
fmt:
	cargo fmt --all

# Check formatting
fmt-check:
	cargo fmt --all -- --check

# Run all checks (format + lint + test)
check: fmt-check lint test

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/
	rm -rf dist/
	rm -rf *.egg-info/
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true

# Install locally
install:
	cargo install --path .

# Build documentation
docs:
	cargo doc --no-deps --open

# Run benchmarks
bench:
	cargo bench

# Security audit
audit:
	cargo audit

# Update dependencies
update:
	cargo update

# Generate coverage report
coverage:
	cargo llvm-cov --all-features --lcov --output-path lcov.info
	cargo llvm-cov report --html

# Build Python wheel
wheel:
	maturin build --release

# Run pre-commit hooks
pre-commit:
	pre-commit run --all-files

# Install development dependencies
dev-setup:
	rustup component add rustfmt clippy llvm-tools-preview
	cargo install cargo-audit cargo-llvm-cov
	pip install pre-commit maturin
	pre-commit install

# Show help
help:
	@echo "agent-precommit development targets:"
	@echo ""
	@echo "  make build        Build debug version"
	@echo "  make release      Build release version"
	@echo "  make test         Run all tests"
	@echo "  make lint         Run Clippy lints"
	@echo "  make fmt          Format code"
	@echo "  make check        Run all checks (fmt + lint + test)"
	@echo "  make clean        Clean build artifacts"
	@echo "  make install      Install locally"
	@echo "  make docs         Build and open documentation"
	@echo "  make bench        Run benchmarks"
	@echo "  make coverage     Generate coverage report"
	@echo "  make wheel        Build Python wheel"
	@echo "  make dev-setup    Install development dependencies"
	@echo "  make help         Show this help"
