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
1. `APC_MODE` environment variable (explicit override)
2. `AGENT_MODE=1` environment variable
3. Known agent env vars (`CLAUDE_CODE`, `CURSOR_SESSION`, `AIDER_MODEL`, etc.)
4. Custom agent env vars from config (`detection.agent_env_vars`)
5. CI environment (`GITHUB_ACTIONS`, `GITLAB_CI`, `CI`, etc.)
6. No TTY (non-interactive terminal)
7. Default: human

## Configuration

`agent-precommit.toml`:

```toml
[detection]
agent_env_vars = ["MY_AGENT"]  # Custom env vars that trigger agent mode

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

## Using with pre-commit Framework

`agent-precommit` is designed to work alongside the [pre-commit](https://pre-commit.com/) framework, not replace it. Here's how they interact:

### How It Works

1. **`apc install` replaces the git hook** - When you run `apc install`, it installs its own `.git/hooks/pre-commit` script that calls `apc run`. If a pre-commit framework hook already exists, it backs it up to `pre-commit.bak`.

2. **`apc` wraps pre-commit as a check** - The built-in `pre-commit` and `pre-commit-all` checks call the `pre-commit` CLI tool:
   - `pre-commit` → Runs `pre-commit run` (staged files only, for humans)
   - `pre-commit-all` → Runs `pre-commit run --all-files` (for agents)

3. **Your `.pre-commit-config.yaml` stays unchanged** - All your existing pre-commit hooks continue to work exactly as before.

### Setup with Existing pre-commit

```bash
# If you already have .pre-commit-config.yaml
apc init        # Auto-detects and enables integration
apc install     # Replaces pre-commit's hook with apc's hook

# Your flow stays the same
git add .
git commit      # apc runs, which runs pre-commit + other checks
```

### What Runs When

| Mode | What Happens |
|------|--------------|
| Human | `apc` → `pre-commit run` (staged files only) |
| Agent | `apc` → `pre-commit run --all-files` + tests + build + merge check |

### Reverting to pre-commit Only

```bash
apc uninstall                           # Removes apc hook
mv .git/hooks/pre-commit.bak .git/hooks/pre-commit  # Restore backup
# Or: pre-commit install                # Reinstall pre-commit's hook
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
apc config                # Show config file location
apc completions bash      # Generate shell completions (bash/zsh/fish)
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
