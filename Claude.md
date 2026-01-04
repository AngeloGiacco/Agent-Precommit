# agent-precommit: Architecture & Design Spec

## Problem Statement

Coding agents (Claude Code, Cursor, Aider, Copilot, Devin, etc.) have fundamentally different commit patterns than human developers:

|Dimension          |Human                   |Agent                                   |
|-------------------|------------------------|----------------------------------------|
|Commit frequency   |Many small commits      |Usually once at completion              |
|Latency sensitivity|High (breaks flow state)|None (asynchronous)                     |
|Cost of CI failure |Low (quick local fix)   |High (context window bloat, token waste)|
|Goal               |Fast iteration          |Merge-ready output                      |

Standard pre-commit hooks optimize for humans. This causes agents to produce commits that fail CI, triggering review cycles that bloat context and waste resources.

**Solution**: A pre-commit system that detects the committer type and routes to appropriate checks.

-----

## Naming

**Name: `agent-precommit`** (alias: `apc`)

This name works because:

- Clear intent: it’s about agent-aware pre-commit hooks
- Natural word order (agent-first, describes what it’s for)
- Doesn’t conflict with existing `pre-commit` framework
- `apc` is a reasonable short alias

-----

## Implementation Language

**Recommendation: Rust**

|Language|Pros                                                                                                                                        |Cons                                                                   |
|--------|--------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
|**Rust**|Single binary, no runtime deps, fast startup, modern CLI ecosystem (clap), cross-platform, growing dev-tools adoption (ruff, ripgrep, delta)|Steeper learning curve, slower development                             |
|Go      |Single binary, simple, fast compile                                                                                                         |Less rich CLI ecosystem                                                |
|Python  |Easy to write, good ecosystem                                                                                                               |Requires Python runtime, startup overhead, virtualenv complexity       |
|Bash    |Universal on Unix                                                                                                                           |Limited on Windows, hard to maintain complex logic, poor error handling|
|Node.js |Good if targeting JS projects                                                                                                               |Requires Node runtime, node_modules bloat                              |

Rust is the right choice for a tool intended to become a standard because:

1. **Zero runtime dependencies** - Works on any machine without installing Python/Node/etc.
1. **Fast startup** - Pre-commit hooks run on every commit; milliseconds matter for human mode
1. **Single binary distribution** - `curl | sh` install, Homebrew, cargo install
1. **Cross-platform** - Windows/Mac/Linux from one codebase
1. **Ecosystem precedent** - Developers trust Rust CLI tools (ripgrep, bat, fd, ruff)

-----

## Integration with Existing Pre-commit Frameworks

Many projects already use the Python `pre-commit` framework (https://pre-commit.com). `agent-precommit` is designed to integrate with it, not replace it.

### Integration Strategies

#### Strategy 1: agent-precommit as a wrapper (Recommended)

`agent-precommit` becomes the git hook and calls `pre-commit` internally:

```
git commit
    │
    ▼
.git/hooks/pre-commit (installed by agent-precommit)
    │
    ▼
agent-precommit (detects mode)
    │
    ├─── Human mode ───▶ pre-commit run (standard hooks)
    │
    └─── Agent mode ────▶ pre-commit run (standard hooks)
                              +
                         agent-precommit extra checks
                              - no-merge-conflicts
                              - full test suite
                              - security scan
                              - build verify
```

**Config example:**

```toml
# agent-precommit.toml

[integration]
# Use existing pre-commit framework for base checks
pre_commit = true
pre_commit_command = "pre-commit run --all-files"  # Agent mode
pre_commit_command_human = "pre-commit run"         # Human mode (staged only)

[human]
# Human mode: just run pre-commit (fast, staged files)
checks = ["pre-commit"]

[agent]
# Agent mode: run pre-commit + additional thorough checks
checks = [
    "pre-commit",           # Delegates to .pre-commit-config.yaml
    "no-merge-conflicts",   # Extra: check for conflicts with main
    "test-all",             # Extra: full test suite
    "security-scan",        # Extra: gitleaks, etc.
    "build-verify",         # Extra: ensure build works
]
```

#### Strategy 2: agent-precommit as a pre-commit hook

Add `agent-precommit` as a hook within the existing `.pre-commit-config.yaml`:

```yaml
# .pre-commit-config.yaml

repos:
  # Standard hooks (run in both modes)
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.5.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer

  - repo: https://github.com/psf/black
    rev: 24.1.0
    hooks:
      - id: black

  # agent-precommit for additional agent-mode checks
  - repo: https://github.com/yourorg/agent-precommit
    rev: v1.0.0
    hooks:
      - id: agent-precommit
        stages: [pre-commit]
        always_run: true
```

The hook internally detects mode and runs additional checks only in agent mode.

#### Strategy 3: Parallel hook directories

Use git’s `core.hooksPath` to switch between hook sets:

```bash
# Human mode (default)
git config core.hooksPath .hooks/human

# Agent mode
git config core.hooksPath .hooks/agent
# Or: git -c core.hooksPath=.hooks/agent commit -m "message"
```

**Directory structure:**

```
.hooks/
├── human/
│   └── pre-commit → runs pre-commit (fast)
└── agent/
    └── pre-commit → runs pre-commit + thorough checks
```

### Recommended Integration Pattern

For most projects with existing `pre-commit` setups:

1. Keep your `.pre-commit-config.yaml` as-is
1. Install `agent-precommit` as the git hook wrapper
1. Configure it to delegate to `pre-commit` plus add agent-specific checks

```toml
# agent-precommit.toml

[integration]
pre_commit = true

[human]
# Delegates to: pre-commit run (staged files, fast)
checks = ["pre-commit"]

[agent]
# Delegates to: pre-commit run --all-files
# Plus runs additional checks
checks = [
    "pre-commit-all",       # pre-commit run --all-files
    "no-merge-conflicts",
    "test-unit",
    "test-integration",
    "security-scan",
    "build-verify",
]

[checks.pre-commit]
run = "pre-commit run"
description = "Run pre-commit hooks on staged files"

[checks.pre-commit-all]
run = "pre-commit run --all-files"
description = "Run pre-commit hooks on all files"

[checks.no-merge-conflicts]
run = """
git fetch origin main --quiet 2>/dev/null || git fetch origin master --quiet 2>/dev/null
BASE=$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD origin/master 2>/dev/null)
if git merge-tree $BASE HEAD origin/main 2>/dev/null | grep -q "^<<<<<<<"; then
    echo "❌ Merge conflicts detected with main"
    exit 1
fi
"""
description = "Check for merge conflicts with main branch"

[checks.test-unit]
run = "pytest"
description = "Run all unit tests"
enabled_if = { file_exists = "pytest.ini", or = { file_exists = "pyproject.toml" } }

[checks.test-integration]
run = "pytest tests/integration/"
description = "Run integration tests"
enabled_if = { dir_exists = "tests/integration" }

[checks.security-scan]
run = "gitleaks detect --source . --no-git"
description = "Scan for secrets"
enabled_if = { command_exists = "gitleaks" }

[checks.build-verify]
run = "python -m build --no-isolation"
description = "Verify package builds"
enabled_if = { file_exists = "pyproject.toml" }
```

-----

## How Agents Trigger Agent Mode

The critical question: how does an agent actually trigger the thorough checks?

### Summary

|Method                                       |Reliability|Best For                    |
|---------------------------------------------|-----------|----------------------------|
|Agent instructions (CLAUDE.md, .cursor/rules)|✅ Best     |Per-project configuration   |
|`AGENT_MODE=1 git commit`                    |✅ Best     |Explicit, any agent         |
|Git alias (`git acommit`)                    |✅ Good     |Convenience, any agent      |
|Wrapper script                               |✅ Good     |Custom workflows            |
|Auto-detection                               |⚠️ Fallback |When explicit isn’t possible|

### Recommended: Explicit Environment Variable

Agents should set `AGENT_MODE=1` when committing:

```bash
AGENT_MODE=1 git commit -m "feat: implement feature"
```

This is the most reliable method. The agent’s instructions or configuration should specify this.

### Agent Configuration Examples

**Claude Code** - Add to `CLAUDE.md` in project root:

```markdown
## Git Workflow
When committing code, always use:
```bash
AGENT_MODE=1 git commit -m "your message"
```

This triggers thorough pre-commit checks (tests, security scan, etc.).

```
**Cursor** - Add to `.cursor/rules`:
```

When committing code with git, always prefix the command with AGENT_MODE=1 to trigger thorough pre-commit checks.
Example: AGENT_MODE=1 git commit -m “message”

```
**Aider** - Add to `.aider.conf.yml`:
```yaml
env:
  AGENT_MODE: "1"
```

**Cline** - Add to workspace instructions:

```
For git commits, set AGENT_MODE=1 environment variable.
```

**Generic (any agent)** - Add to system prompt or project docs:

```
When committing code, always run: AGENT_MODE=1 git commit -m "message"
This ensures thorough checks run before the commit is created.
```

### Alternative: Git Alias

Create a dedicated commit command for agents:

```bash
git config alias.acommit '!AGENT_MODE=1 git commit'
```

Then instruct agents to use `git acommit -m "message"` instead of `git commit`.

### Alternative: Wrapper Script

Create `scripts/agent-commit.sh`:

```bash
#!/bin/bash
AGENT_MODE=1 git commit "$@"
```

Instruct agents to use `./scripts/agent-commit.sh -m "message"`.

### Auto-Detection (Fallback)

If `AGENT_MODE` is not set, `agent-precommit` attempts detection via:

1. **Known agent env vars**: `CLAUDE_CODE`, `CURSOR_SESSION`, `AIDER_*`, `DEVIN_*`, `CLINE_*`
1. **CI detection**: `CI=true`, `GITHUB_ACTIONS`, `GITLAB_CI`
1. **TTY detection**: No stdin/stdout TTY suggests non-interactive (agent)
1. **Parent process**: Check if parent process name contains agent signatures

Auto-detection is a fallback. Explicit `AGENT_MODE=1` is preferred because:

- Agent env vars aren’t standardized
- TTY detection has edge cases (Git GUIs, etc.)
- Explicit is debuggable (`apc detect` shows why)

### Why Not Auto-Detect Everything?

We considered having `agent-precommit` always try to detect agent vs human automatically. Problems:

1. **False positives**: A human in a non-TTY environment (ssh, script) triggers agent mode
1. **False negatives**: A new agent without known env vars gets human mode
1. **Unpredictable**: Users can’t easily understand or debug behavior
1. **No standard**: There’s no universal “I am an AI agent” signal

Explicit `AGENT_MODE=1` is a clear contract. Agents that want thorough checks opt in.

-----

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         agent-precommit                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐ │
│  │   Detector   │────▶│    Router    │────▶│   Check Executor     │ │
│  └──────────────┘     └──────────────┘     └──────────────────────┘ │
│         │                    │                       │               │
│         ▼                    ▼                       ▼               │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐ │
│  │ • Env vars   │     │ • human      │     │ • Parallel executor  │ │
│  │ • TTY check  │     │ • agent      │     │ • Progress reporting │ │
│  │ • Parent PID │     │ • ci         │     │ • Error aggregation  │ │
│  │ • CI detect  │     │              │     │ • pre-commit interop │ │
│  └──────────────┘     └──────────────┘     └──────────────────────┘ │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                        Configuration                                 │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  agent-precommit.toml                                          │ │
│  │  ├── [integration]   # pre-commit framework integration        │ │
│  │  ├── [detection]     # How to detect mode                      │ │
│  │  ├── [human]         # Fast checks config                      │ │
│  │  ├── [agent]         # Thorough checks config                  │ │
│  │  └── [checks.*]      # Individual check definitions            │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Detector

Determines if the commit is from a human or agent.

**Detection priority (first match wins):**

1. `APC_MODE=human|agent` - Explicit override
1. `AGENT_MODE=1` - Generic agent flag
1. Known agent env vars:
- `CLAUDE_CODE`, `ANTHROPIC_PROJECT_ID` (Claude Code)
- `CURSOR_SESSION` (Cursor)
- `AIDER_*` (Aider)
- `CODEX_*` (OpenAI Codex)
- `DEVIN_*` (Devin)
- `CLINE_*` (Cline)
- `CONTINUE_*` (Continue.dev)
1. CI environment:
- `CI=true`, `GITHUB_ACTIONS`, `GITLAB_CI`, `JENKINS_URL`, `BUILDKITE`, etc.
- **Note**: CI should run agent-mode checks (thorough)
1. TTY detection:
- No stdin/stdout TTY + no known GUI = likely agent
1. Parent process heuristics:
- Check `PPID` command name for agent signatures
1. Default: `human`

#### 2. Router

Maps detected mode to check configuration.

```rust
enum Mode {
    Human,  // Fast checks, staged files only
    Agent,  // Thorough checks, full codebase
    CI,     // Same as Agent, possibly with extra reporting
}
```

#### 3. Check Executor

Runs checks with appropriate parallelism and reporting.

**Human mode**:

- Sequential for fast feedback
- Fail fast on first error
- Minimal output

**Agent mode**:

- Parallel where possible (lint + type check can run together)
- Run all checks, aggregate all errors
- Detailed output with timing

-----

## Configuration Format

**File**: `agent-precommit.toml` (project root)

```toml
[detection]
# Override auto-detection
# mode = "human"  # Force human mode
# mode = "agent"  # Force agent mode

# Custom environment variables to detect as agent
agent_env_vars = ["MY_CUSTOM_AGENT"]

[integration]
# Integration with existing pre-commit framework
pre_commit = true  # Enable pre-commit integration
pre_commit_path = ".pre-commit-config.yaml"  # Path to config (default)

[human]
# Checks to run in human mode (fast, non-blocking)
checks = ["pre-commit"]
timeout = "30s"
fail_fast = true

[agent]
# Checks to run in agent mode (thorough, merge-ready)
checks = [
    "pre-commit-all",
    "no-merge-conflicts",
    "test-unit",
    "test-integration",
    "security-scan",
    "build-verify",
]
timeout = "15m"
fail_fast = false  # Run all, report all failures

# Parallel execution groups (checks in same group run in parallel)
parallel_groups = [
    ["pre-commit-all"],
    ["no-merge-conflicts", "security-scan"],
    ["test-unit"],
    ["test-integration"],
    ["build-verify"],
]

#──────────────────────────────────────────────────────────────────────
# Built-in Check Definitions (can be overridden)
#──────────────────────────────────────────────────────────────────────

[checks.pre-commit]
description = "Run pre-commit on staged files"
run = "pre-commit run"

[checks.pre-commit-all]
description = "Run pre-commit on all files"
run = "pre-commit run --all-files"

[checks.no-merge-conflicts]
description = "Ensure no merge conflicts with main/master"
run = """
git fetch origin main --quiet 2>/dev/null || git fetch origin master --quiet
MAIN_BRANCH=$(git rev-parse --verify origin/main 2>/dev/null && echo "main" || echo "master")
BASE=$(git merge-base HEAD origin/$MAIN_BRANCH)
if git merge-tree $BASE HEAD origin/$MAIN_BRANCH | grep -q "^<<<<<<<"; then
    echo "❌ Would conflict with $MAIN_BRANCH"
    exit 1
fi
echo "✓ No conflicts with $MAIN_BRANCH"
"""

[checks.test-unit]
description = "Run unit tests"
run = "pytest -x -q"
enabled_if = { any = [
    { file_exists = "pytest.ini" },
    { file_exists = "pyproject.toml" },
    { file_exists = "setup.py" },
]}

[checks.test-integration]
description = "Run integration tests"
run = "pytest tests/integration/ -v"
enabled_if = { dir_exists = "tests/integration" }

[checks.security-scan]
description = "Scan for secrets and vulnerabilities"
run = "gitleaks detect --source . --no-git"
enabled_if = { command_exists = "gitleaks" }

[checks.build-verify]
description = "Verify package builds"
run = "python -m build --no-isolation"
enabled_if = { file_exists = "pyproject.toml" }
```

-----

## Recommended Agent Checks

These should be the **default agent checks** for a comprehensive “merge-ready” guarantee:

### 1. Run Existing Pre-commit Hooks (All Files)

```bash
pre-commit run --all-files
```

**Why**: Leverages your existing setup. Don’t reinvent the wheel.

### 2. No Merge Conflicts with Main

```bash
git fetch origin main --quiet
MERGE_BASE=$(git merge-base HEAD origin/main)
CONFLICTS=$(git merge-tree $MERGE_BASE HEAD origin/main | grep -c "^<<<<<<<" || true)
if [ "$CONFLICTS" -gt 0 ]; then
    echo "❌ Would have merge conflicts with main"
    exit 1
fi
```

**Why**: Agents work asynchronously. Main may have moved. Catching this prevents failed merges.

### 3. All Unit Tests Pass

```bash
pytest  # or npm test, go test, cargo test, etc.
```

**Why**: Non-negotiable. Agents should never commit code that breaks tests.

### 4. All Integration/E2E Tests Pass

```bash
pytest tests/integration/
```

**Why**: Catches issues unit tests miss. Agents have time to run these.

### 5. Security Scan

```bash
# Secrets detection
gitleaks detect --source . --no-git

# SAST (optional but recommended)
semgrep scan --config auto --error

# Dependency vulnerabilities
pip-audit  # or npm audit, cargo audit
```

**Why**: Agents may inadvertently commit secrets or introduce vulnerable patterns.

### 6. Build Verification

```bash
python -m build  # or npm run build, cargo build, etc.
```

**Why**: Ensures the code actually compiles/bundles. Catches import errors.

### 7. No Debug Artifacts

```bash
# Check for common debug statements
! grep -rn "breakpoint()\|pdb\|ipdb\|print(" src/ --include="*.py" | grep -v "# apc-ignore"
! grep -rn "console\.log\|debugger" src/ --include="*.ts" --include="*.js" | grep -v "// apc-ignore"
```

**Why**: Agents often add debug statements while working. Should be removed.

### 8. Type Check (if applicable)

```bash
mypy .  # or pyright, tsc --noEmit
```

**Why**: Type errors are easy to introduce when agents modify multiple files.

### 9. Coverage Threshold (Optional)

```bash
pytest --cov=src --cov-fail-under=80
```

**Why**: Ensures agents write tests for new code.

-----

## CLI Interface

```bash
# Installation
cargo install agent-precommit
# or
brew install agent-precommit
# or
curl -fsSL https://agent-precommit.dev/install.sh | sh

# Initialize in a project
apc init                    # Creates agent-precommit.toml with defaults
apc init --preset=python    # Use Python preset (integrates with pre-commit)
apc init --preset=node      # Use Node.js preset
apc init --preset=rust      # Use Rust preset

# Install git hook
apc install                 # Creates .git/hooks/pre-commit

# Manual run
apc run                     # Auto-detect mode and run
apc run --mode=human        # Force human mode
apc run --mode=agent        # Force agent mode
apc run --check=test-unit   # Run specific check

# Utilities
apc detect                  # Show detected mode and why
apc list                    # List all configured checks
apc validate                # Validate config file
```

-----

## Distribution

1. **Cargo**: `cargo install agent-precommit`
1. **Homebrew**: `brew install agent-precommit`
1. **pip** (wrapper): `pip install agent-precommit` (downloads binary)
1. **Direct download**: GitHub releases with binaries for all platforms
1. **Shell installer**: `curl -fsSL https://agent-precommit.dev/install.sh | sh`

-----

## Migration from Pure pre-commit

For projects currently using only `pre-commit`:

```bash
# 1. Install agent-precommit
brew install agent-precommit

# 2. Initialize (detects existing .pre-commit-config.yaml)
apc init
# This creates agent-precommit.toml with pre_commit = true

# 3. Replace the git hook
apc install
# Backs up existing .git/hooks/pre-commit
# Installs agent-precommit as the hook

# 4. Done! 
# - Human commits: runs `pre-commit run` (same as before)
# - Agent commits: runs `pre-commit run --all-files` + extra checks
```

-----

## Future Considerations

1. **Preset library**: Community-contributed presets for common stacks
1. **Remote check execution**: Run heavy checks on remote machines
1. **Caching**: Skip checks if relevant files haven’t changed
1. **Deep pre-commit integration**: Parse .pre-commit-config.yaml and run hooks natively
1. **Metrics/telemetry**: Track check durations, failure rates (opt-in)
1. **Editor integration**: VS Code extension to show check status

-----

## Success Criteria

The tool is successful when:

1. Agent commits pass CI on first push >95% of the time
1. Human developers don’t notice any slowdown
1. Setup takes <5 minutes for common stacks
1. Zero runtime dependencies required
1. Seamless integration with existing pre-commit setups
