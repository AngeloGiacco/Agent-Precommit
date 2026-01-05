# agent-precommit

Pre-commit hooks that run fast checks for humans and thorough checks for AI agents.

```
Human commit: ~3s          Agent commit: ~5min
├─ pre-commit (staged)     ├─ pre-commit (all files)
└─ done                    ├─ tests, build, security
                           └─ merge conflict check
```

## Why

AI agents commit once when done. If that commit fails CI, you waste a round-trip fixing it. `agent-precommit` runs CI-level checks locally before the commit, so agent commits are merge-ready.

## Setup

```bash
cargo install agent-precommit

cd your-project
apc init      # Creates agent-precommit.toml
apc install   # Installs git hook
```

If you have an existing `.pre-commit-config.yaml`, `apc init` detects it and wraps it automatically.

## Triggering Agent Mode

Set `AGENT_MODE=1` when committing from an agent. Add this to your agent's instructions (CLAUDE.md, .cursor/rules, etc.):

```
When committing, use: AGENT_MODE=1 git commit -m "message"
```

Or create a git alias:

```bash
git config alias.acommit '!AGENT_MODE=1 git commit'
```

### Auto-detection

If `AGENT_MODE` isn't set, the tool checks (in order):
1. `APC_MODE` environment variable
2. Known agent env vars (`CLAUDE_CODE`, `CURSOR_SESSION`, `AIDER_MODEL`, etc.)
3. CI environment (`GITHUB_ACTIONS`, `CI`, etc.)
4. No TTY (non-interactive terminal)
5. Default: human

## Configuration

`agent-precommit.toml`:

```toml
[integration]
pre_commit = true  # Wrap existing .pre-commit-config.yaml

[human]
checks = ["pre-commit"]
timeout = "30s"

[agent]
checks = [
    "pre-commit-all",
    "no-merge-conflicts",
    "test-unit",
    "build-verify",
]
timeout = "15m"

[checks.test-unit]
run = "cargo test"

[checks.build-verify]
run = "cargo build --release"
```

### Presets

```bash
apc init --preset=python   # ruff, pytest, mypy
apc init --preset=node     # eslint, jest, tsc
apc init --preset=rust     # cargo fmt, clippy, test
apc init --preset=go       # gofmt, golangci-lint, go test
```

## CLI

```bash
apc init                  # Create config
apc install               # Install git hook
apc uninstall             # Remove hook
apc run                   # Run checks (auto-detect mode)
apc run --mode=agent      # Force agent mode
apc run --check=test-unit # Run single check
apc detect                # Show detected mode
apc list                  # List checks
apc validate              # Validate config
```

## Environment Variables

| Variable     | Description                           |
|--------------|---------------------------------------|
| `APC_MODE`   | Force mode: `human`, `agent`, or `ci` |
| `AGENT_MODE` | Set to `1` for agent mode             |
| `APC_SKIP`   | Set to `1` to skip all checks         |

## Skipping

```bash
git commit --no-verify -m "skip checks"
APC_SKIP=1 git commit -m "skip checks"
```

## License

MIT
