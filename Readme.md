# agent-precommit

**Smart pre-commit hooks for humans and AI coding agents.**

Humans commit often and need fast checks. Agents commit once and need thorough checks. `agent-precommit` auto-detects which is which and runs the right checks.

```
Human commit: ~3 seconds    Agent commit: ~5 minutes
├─ pre-commit (staged)      ├─ merge conflict check
└─ done                     ├─ pre-commit (all files)
                            ├─ all unit tests
                            ├─ integration tests
                            ├─ security scan
                            └─ build verification
```

## Why?

AI coding agents (Claude Code, Cursor, Aider, etc.) work asynchronously and commit once when done. If that commit fails CI, fixing it requires another round-trip that:

- Bloats the agent’s context window
- Wastes tokens and money
- Slows down delivery

**agent-precommit** ensures agent commits are **merge-ready** by running comprehensive checks locally before commit.

## Install

```bash
# Homebrew
brew install agent-precommit

# Cargo
cargo install agent-precommit

# pip (wrapper)
pip install agent-precommit

# Direct download
curl -fsSL https://agent-precommit.dev/install.sh | sh
```

## Quick Start

```bash
cd your-project

# Initialize (auto-detects your stack and existing pre-commit config)
apc init

# Install the git hook
apc install

# Done! Commits now auto-detect human vs agent
```

## Integration with pre-commit Framework

`agent-precommit` is designed to work **with** your existing `pre-commit` setup, not replace it.

### If you already use pre-commit

```bash
# Your existing .pre-commit-config.yaml stays exactly as-is
# agent-precommit wraps it and adds agent-specific checks

apc init  # Detects .pre-commit-config.yaml automatically
apc install
```

**What happens:**

- **Human commits**: Runs `pre-commit run` (staged files, fast) — same as before
- **Agent commits**: Runs `pre-commit run --all-files` + additional checks (tests, security, etc.)

### If you don’t use pre-commit yet

```bash
apc init --preset=python  # or node, rust, go
apc install
```

This creates a standalone `agent-precommit.toml` with sensible defaults.

## How It Works

When you run `git commit`, agent-precommit:

1. **Detects** if you’re a human or an agent
1. **Routes** to the appropriate check suite
1. **Runs** checks (fast for humans, thorough for agents)
1. **Reports** results and blocks commit if checks fail

### Detection

The tool detects agents via (in order):

1. `APC_MODE=agent` environment variable
1. `AGENT_MODE=1` environment variable
1. Known agent env vars (`CLAUDE_CODE`, `CURSOR_SESSION`, etc.)
1. CI environment (`GITHUB_ACTIONS`, `GITLAB_CI`, etc.)
1. No TTY (non-interactive terminal)
1. Default: human

### Triggering Agent Mode

Agent mode must be triggered explicitly. The most reliable method is setting `AGENT_MODE=1`.

|Method              |Reliability|Setup                                |
|--------------------|-----------|-------------------------------------|
|Agent instructions  |✅ Best     |Add to CLAUDE.md, .cursor/rules, etc.|
|Environment variable|✅ Best     |`AGENT_MODE=1 git commit`            |
|Git alias           |✅ Good     |`git acommit` → triggers agent mode  |
|Wrapper script      |✅ Good     |`./scripts/agent-commit.sh`          |
|Auto-detection      |⚠️ Fallback |Checks env vars, TTY, parent process |

**Option 1: Configure your agent (Recommended)**

Add to your agent’s instructions or config file:

*Claude Code* - in `CLAUDE.md`:

```markdown
## Git Commits
When committing, always use:
AGENT_MODE=1 git commit -m "message"
```

*Cursor* - in `.cursor/rules` or settings:

```
When running git commit, prefix with AGENT_MODE=1
```

*Aider* - in `.aider.conf.yml`:

```yaml
env:
  AGENT_MODE: "1"
```

*Cline* - in workspace instructions:

```
For git commits, set AGENT_MODE=1 environment variable.
```

*Any agent* - in system prompt or project docs:

```
When committing code, always run: AGENT_MODE=1 git commit -m "message"
```

**Option 2: Per-commit environment variable**

```bash
AGENT_MODE=1 git commit -m "feat: implement feature"
```

**Option 3: Git alias**

```bash
# Setup once
git config alias.acommit '!AGENT_MODE=1 git commit'

# Then tell agent to use:
git acommit -m "message"
```

**Option 4: Wrapper script**

Create `scripts/agent-commit.sh`:

```bash
#!/bin/bash
AGENT_MODE=1 git commit "$@"
```

Then instruct agents to use `./scripts/agent-commit.sh -m "message"`.

**Option 5: Shell profile (use with caution)**

```bash
# In ~/.bashrc or ~/.zshrc
export AGENT_MODE=1  # Warning: affects ALL commits including human ones
```

**Option 6: Auto-detection (fallback)**

If no env var is set, `agent-precommit` attempts auto-detection via:

- Known agent env vars (`CLAUDE_CODE`, `CURSOR_SESSION`, `AIDER_*`, `DEVIN_*`, `CLINE_*`)
- CI environment (`GITHUB_ACTIONS`, `CI=true`, `GITLAB_CI`)
- No TTY + no GUI
- Parent process heuristics

Auto-detection works but explicit `AGENT_MODE=1` is more reliable and debuggable.

## Configuration

Configuration lives in `agent-precommit.toml` at your project root.

### With Existing pre-commit

```toml
[integration]
pre_commit = true  # Use your existing .pre-commit-config.yaml

[human]
checks = ["pre-commit"]  # Just run pre-commit (staged files)
timeout = "30s"

[agent]
checks = [
    "pre-commit-all",       # pre-commit run --all-files
    "no-merge-conflicts",   # Check for conflicts with main
    "test-unit",            # Run all unit tests
    "test-integration",     # Run integration tests  
    "security-scan",        # Scan for secrets
    "build-verify",         # Verify build works
]
timeout = "15m"
```

### Standalone (No pre-commit)

```toml
[human]
checks = ["lint-staged", "format-staged"]
timeout = "30s"

[agent]
checks = [
    "no-merge-conflicts",
    "lint-all",
    "type-check",
    "test-unit",
    "test-integration",
    "security-scan",
    "build-verify",
]
timeout = "15m"

[checks.lint-staged]
run = "ruff check $(git diff --cached --name-only | grep '.py$')"

[checks.lint-all]
run = "ruff check . --fix"

[checks.type-check]
run = "mypy ."

[checks.test-unit]
run = "pytest -x"

[checks.test-integration]
run = "pytest tests/integration/"
enabled_if = { dir_exists = "tests/integration" }

[checks.security-scan]
run = "gitleaks detect --source . --no-git"
enabled_if = { command_exists = "gitleaks" }

[checks.build-verify]
run = "python -m build"
enabled_if = { file_exists = "pyproject.toml" }

[checks.no-merge-conflicts]
run = """
git fetch origin main --quiet 2>/dev/null || git fetch origin master --quiet 2>/dev/null
MAIN=$(git rev-parse --verify origin/main 2>/dev/null && echo main || echo master)
BASE=$(git merge-base HEAD origin/$MAIN)
if git merge-tree $BASE HEAD origin/$MAIN | grep -q "^<<<<<<<"; then
    echo "❌ Merge conflicts with $MAIN"
    exit 1
fi
"""
```

## Presets

Initialize with a preset for your stack:

```bash
apc init --preset=python     # Python (ruff, pytest, mypy, pre-commit integration)
apc init --preset=node       # Node.js/TypeScript (eslint, jest, tsc)
apc init --preset=rust       # Rust (cargo fmt, clippy, test)
apc init --preset=go         # Go (gofmt, golangci-lint, go test)
```

## Recommended Agent Checks

For maximum “merge-ready” confidence:

|Check                 |Why                                           |
|----------------------|----------------------------------------------|
|**no-merge-conflicts**|Agents work async; main may have moved        |
|**pre-commit-all**    |Run your existing hooks on all files          |
|**test-unit**         |Non-negotiable; never commit broken tests     |
|**test-integration**  |Catches issues unit tests miss                |
|**security-scan**     |Detect accidentally committed secrets         |
|**build-verify**      |Ensure code compiles/packages correctly       |
|**type-check**        |Type errors are easy to introduce across files|
|**no-debug-artifacts**|Remove print(), console.log, debugger         |

## CLI Reference

```bash
# Initialize
apc init                      # Create config, auto-detect stack
apc init --preset=python      # Use specific preset
apc init --force              # Overwrite existing config

# Install hook
apc install                   # Install git pre-commit hook
apc uninstall                 # Remove git hook

# Run checks
apc run                       # Auto-detect mode, run checks
apc run --mode=human          # Force human mode
apc run --mode=agent          # Force agent mode
apc run --check=test-unit     # Run specific check only

# Utilities  
apc detect                    # Show detected mode and reasoning
apc list                      # List all configured checks
apc validate                  # Validate config file
```

## Environment Variables

|Variable     |Description                          |
|-------------|-------------------------------------|
|`APC_MODE`   |Force mode: `human`, `agent`, or `ci`|
|`AGENT_MODE` |Set to `1` to trigger agent mode     |
|`APC_SKIP`   |Set to `1` to skip all checks        |
|`APC_TIMEOUT`|Override timeout (e.g., `5m`, `300s`)|

## Skipping Checks

```bash
# Skip all checks (escape hatch)
git commit --no-verify -m "emergency fix"

# Skip via environment
APC_SKIP=1 git commit -m "skip checks"

# Ignore specific line in code
print("debug")  # apc-ignore
```

## Migration from pre-commit Only

If you’re currently using only the `pre-commit` framework:

```bash
# 1. Install agent-precommit
brew install agent-precommit

# 2. Initialize (detects your .pre-commit-config.yaml)
apc init
# Creates agent-precommit.toml with pre_commit = true

# 3. Replace the git hook  
apc install
# Your .pre-commit-config.yaml is unchanged
# agent-precommit now wraps it

# 4. Done!
# Human commits work exactly as before
# Agent commits get extra thorough checks
```

## GitHub Actions Integration

```yaml
# .github/workflows/ci.yml
- name: Run agent-precommit checks
  run: apc run --mode=agent
```

This ensures CI runs the same checks as agent commits, keeping them in sync.

## FAQ

**Q: Does this replace my .pre-commit-config.yaml?**

A: No! It wraps it. Your existing pre-commit hooks keep working. agent-precommit just adds mode-awareness and extra agent checks.

**Q: What if detection is wrong?**

A: Use `APC_MODE=human` or `APC_MODE=agent` to override. Run `apc detect` to see why a mode was chosen.

**Q: Can I run only specific checks?**

A: Yes: `apc run --check=test-unit`

**Q: My agent checks are too slow.**

A: Configure parallel execution:

```toml
[agent]
parallel_groups = [
    ["pre-commit-all", "no-merge-conflicts"],
    ["test-unit"],
    ["test-integration", "security-scan"],
]
```

**Q: How do I add a custom check?**

A: Add it to `[checks.*]`:

```toml
[checks.my-check]
run = "my-command --flag"
description = "My custom check"
enabled_if = { file_exists = "my-config.json" }
```

Then add `"my-check"` to your `[agent].checks` list.

## License

MIT
