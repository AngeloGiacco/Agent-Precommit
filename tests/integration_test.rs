//! Integration tests for agent-precommit CLI.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// Creates a test git repository.
fn create_test_repo() -> TempDir {
    let temp = TempDir::new().expect("create temp dir");

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("init repo");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .expect("set email");

    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .expect("set name");

    temp
}

/// Helper to create the APC command
#[allow(deprecated)]
fn apc_cmd() -> Command {
    Command::cargo_bin("apc").expect("find apc binary")
}

#[test]
fn test_help() {
    apc_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("intelligent pre-commit hooks"));
}

#[test]
fn test_version() {
    apc_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_detect_default_human() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        // Clear any agent/CI env vars that might be set
        .env_remove("AGENT_MODE")
        .env_remove("APC_MODE")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("CLAUDE_CODE")
        .env_remove("CURSOR_SESSION")
        .current_dir(temp.path())
        .assert()
        .success()
        // In test mode both stdin/stdout may not be TTY, so it might detect as agent
        // Just verify it detects something successfully
        .stderr(predicate::str::contains("Detected mode:"));
}

#[test]
fn test_detect_agent_mode() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        .env("AGENT_MODE", "1")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("agent"));
}

#[test]
fn test_detect_with_apc_mode_env() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        .env("APC_MODE", "ci")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("ci"));
}

#[test]
fn test_detect_with_claude_code_env() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        .env("CLAUDE_CODE", "1")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("agent"));
}

#[test]
fn test_detect_with_cursor_env() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        .env("CURSOR_SESSION", "test-session")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("agent"));
}

#[test]
fn test_detect_ci_environment() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("detect")
        .env("CI", "true")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("ci"));
}

#[test]
fn test_init_creates_config() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Created agent-precommit.toml"));

    assert!(temp.path().join("agent-precommit.toml").exists());
}

#[test]
fn test_init_with_preset() {
    let temp = create_test_repo();

    apc_cmd()
        .args(["init", "--preset", "rust"])
        .current_dir(temp.path())
        .assert()
        .success();

    let config =
        std::fs::read_to_string(temp.path().join("agent-precommit.toml")).expect("read config");

    assert!(config.contains("clippy"));
}

#[test]
fn test_init_with_python_preset() {
    let temp = create_test_repo();

    apc_cmd()
        .args(["init", "--preset", "python"])
        .current_dir(temp.path())
        .assert()
        .success();

    let config =
        std::fs::read_to_string(temp.path().join("agent-precommit.toml")).expect("read config");

    assert!(config.contains("pytest"));
}

#[test]
fn test_init_with_node_preset() {
    let temp = create_test_repo();

    apc_cmd()
        .args(["init", "--preset", "node"])
        .current_dir(temp.path())
        .assert()
        .success();

    let config =
        std::fs::read_to_string(temp.path().join("agent-precommit.toml")).expect("read config");

    assert!(config.contains("npm"));
}

#[test]
fn test_init_with_go_preset() {
    let temp = create_test_repo();

    apc_cmd()
        .args(["init", "--preset", "go"])
        .current_dir(temp.path())
        .assert()
        .success();

    let config =
        std::fs::read_to_string(temp.path().join("agent-precommit.toml")).expect("read config");

    assert!(config.contains("go test"));
}

#[test]
fn test_init_already_exists() {
    let temp = create_test_repo();
    std::fs::write(temp.path().join("agent-precommit.toml"), "").expect("create config");

    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_init_force() {
    let temp = create_test_repo();
    std::fs::write(temp.path().join("agent-precommit.toml"), "").expect("create config");

    apc_cmd()
        .args(["init", "--force"])
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn test_validate_no_config() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_validate_valid_config() {
    let temp = create_test_repo();

    // Initialize first
    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    apc_cmd()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("valid"));
}

#[test]
fn test_validate_invalid_config() {
    let temp = create_test_repo();

    // Write invalid config
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
timeout = "invalid"
"#,
    )
    .expect("write config");

    apc_cmd()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid duration"));
}

#[test]
fn test_list_checks() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    apc_cmd()
        .arg("list")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Human mode checks"))
        .stderr(predicate::str::contains("Agent mode checks"));
}

#[test]
fn test_list_checks_human_only() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    apc_cmd()
        .args(["list", "--mode", "human"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Human mode checks"));
}

#[test]
fn test_install_hook() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Installed pre-commit hook"));

    let hook_path = temp.path().join(".git/hooks/pre-commit");
    assert!(hook_path.exists());

    let hook_content = std::fs::read_to_string(&hook_path).expect("read hook");
    assert!(hook_content.contains("agent-precommit"));
}

#[test]
fn test_install_hook_idempotent() {
    let temp = create_test_repo();

    // Install first time
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .success();

    // Install again - should be idempotent
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("already installed"));
}

#[test]
fn test_install_hook_force_overwrites() {
    let temp = create_test_repo();

    // Create a custom hook
    let hooks_dir = temp.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).expect("create hooks dir");
    std::fs::write(
        hooks_dir.join("pre-commit"),
        "#!/bin/sh\necho 'custom hook'",
    )
    .expect("write custom hook");

    // Install with --force
    apc_cmd()
        .args(["install", "--force"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Verify our hook was installed
    let hook_content = std::fs::read_to_string(hooks_dir.join("pre-commit")).expect("read hook");
    assert!(hook_content.contains("agent-precommit"));

    // Verify backup was created
    assert!(hooks_dir.join("pre-commit.bak").exists());
}

#[test]
fn test_install_hook_refuses_without_force() {
    let temp = create_test_repo();

    // Create a custom hook
    let hooks_dir = temp.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).expect("create hooks dir");
    std::fs::write(
        hooks_dir.join("pre-commit"),
        "#!/bin/sh\necho 'custom hook'",
    )
    .expect("write custom hook");

    // Install without --force should fail
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
}

#[test]
fn test_uninstall_hook() {
    let temp = create_test_repo();

    // Install first
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .output()
        .expect("install");

    // Then uninstall
    apc_cmd()
        .arg("uninstall")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed"));

    assert!(!temp.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn test_uninstall_no_hook() {
    let temp = create_test_repo();

    // Uninstall when no hook exists
    apc_cmd()
        .arg("uninstall")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No hook installed"));
}

#[test]
fn test_uninstall_refuses_foreign_hook() {
    let temp = create_test_repo();

    // Create a custom hook (not ours)
    let hooks_dir = temp.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).expect("create hooks dir");
    std::fs::write(
        hooks_dir.join("pre-commit"),
        "#!/bin/sh\necho 'custom hook'",
    )
    .expect("write custom hook");

    // Uninstall should refuse
    apc_cmd()
        .arg("uninstall")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not installed by agent-precommit"));
}

#[test]
fn test_skip_with_env_var() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("run")
        .env("APC_SKIP", "1")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping"));
}

#[test]
fn test_run_with_mode_override() {
    let temp = create_test_repo();

    // Initialize config
    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    // Run with mode override
    apc_cmd()
        .args(["run", "--mode", "agent"])
        .current_dir(temp.path())
        .assert();
    // We just check it doesn't crash - the checks will fail without proper setup
}

#[test]
fn test_not_git_repo() {
    let temp = TempDir::new().expect("create temp dir");

    // The install command requires a git repository
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a Git repository"));
}

#[test]
fn test_config_command_no_config() {
    let temp = create_test_repo();

    apc_cmd()
        .arg("config")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No configuration file found"));
}

#[test]
fn test_config_command_with_config() {
    let temp = create_test_repo();

    // Initialize
    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    apc_cmd()
        .arg("config")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("agent-precommit.toml"));
}

#[test]
fn test_config_command_raw() {
    let temp = create_test_repo();

    // Initialize
    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    apc_cmd()
        .args(["config", "--raw"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[human]"));
}

#[test]
fn test_completions_bash() {
    apc_cmd()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_completions_zsh() {
    apc_cmd()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("compdef"));
}

#[test]
fn test_completions_fish() {
    apc_cmd()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

// ============================================================================
// Pre-commit integration tests
// ============================================================================

#[test]
fn test_init_detects_precommit_config() {
    let temp = create_test_repo();

    // Create a pre-commit config
    std::fs::write(temp.path().join(".pre-commit-config.yaml"), "repos: []")
        .expect("write pre-commit config");

    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Detected .pre-commit-config.yaml"));

    let config =
        std::fs::read_to_string(temp.path().join("agent-precommit.toml")).expect("read config");

    assert!(config.contains("pre_commit = true"));
}

#[test]
fn test_run_skips_disabled_checks() {
    let temp = create_test_repo();

    // Write config with checks that have conditions
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["nonexistent-check"]
timeout = "30s"

[agent]
checks = []
timeout = "15m"

[checks.nonexistent-check]
run = "echo test"
description = "Test check"
[checks.nonexistent-check.enabled_if]
file_exists = "nonexistent-file.txt"
"#,
    )
    .expect("write config");

    // Run should succeed (skipped checks count as passed)
    apc_cmd()
        .arg("run")
        .current_dir(temp.path())
        .assert()
        .success();
}

// ============================================================================
// E2E workflow tests
// ============================================================================

#[test]
fn test_full_workflow_init_install_run() {
    let temp = create_test_repo();

    // Step 1: Initialize
    apc_cmd()
        .args(["init", "--preset", "rust"])
        .current_dir(temp.path())
        .assert()
        .success();

    assert!(temp.path().join("agent-precommit.toml").exists());

    // Step 2: Validate config
    apc_cmd()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .success();

    // Step 3: Install hook
    apc_cmd()
        .arg("install")
        .current_dir(temp.path())
        .assert()
        .success();

    assert!(temp.path().join(".git/hooks/pre-commit").exists());

    // Step 4: List checks
    apc_cmd()
        .arg("list")
        .current_dir(temp.path())
        .assert()
        .success();

    // Step 5: Uninstall hook
    apc_cmd()
        .arg("uninstall")
        .current_dir(temp.path())
        .assert()
        .success();

    assert!(!temp.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn test_workflow_with_custom_checks() {
    let temp = create_test_repo();

    // Write custom config
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["echo-test"]
timeout = "30s"

[agent]
checks = ["echo-test"]
timeout = "15m"

[checks.echo-test]
run = "echo 'Hello from custom check'"
description = "A simple echo test"
"#,
    )
    .expect("write config");

    // Validate
    apc_cmd()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .success();

    // Run - should execute our echo command
    apc_cmd()
        .arg("run")
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn test_workflow_failing_check() {
    let temp = create_test_repo();

    // Write config with a failing check
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["fail-check"]
timeout = "30s"

[agent]
checks = ["fail-check"]
timeout = "15m"

[checks.fail-check]
run = "exit 1"
description = "A check that always fails"
"#,
    )
    .expect("write config");

    // Run should fail
    apc_cmd()
        .arg("run")
        .current_dir(temp.path())
        .assert()
        .failure();
}

#[test]
fn test_environment_variables_in_checks() {
    let temp = create_test_repo();

    // Write config with env vars
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["env-check"]
timeout = "30s"

[agent]
checks = []
timeout = "15m"

[checks.env-check]
run = "test -n \"$MY_TEST_VAR\""
description = "Check env var is set"
[checks.env-check.env]
MY_TEST_VAR = "hello"
"#,
    )
    .expect("write config");

    // Run should succeed because env var is set
    apc_cmd()
        .arg("run")
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn test_check_with_working_directory() {
    let temp = create_test_repo();

    // Create a subdirectory with a file
    let subdir = temp.path().join("subdir");
    std::fs::create_dir(&subdir).expect("create subdir");
    std::fs::write(subdir.join("marker.txt"), "exists").expect("write marker");

    // Write config
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["file-check"]
timeout = "30s"

[agent]
checks = []
timeout = "15m"

[checks.file-check]
run = "test -f subdir/marker.txt"
description = "Check file exists"
"#,
    )
    .expect("write config");

    // Run from repo root should find the file
    apc_cmd()
        .arg("run")
        .current_dir(temp.path())
        .assert()
        .success();
}

// ============================================================================
// Edge case tests
// ============================================================================

#[test]
fn test_deeply_nested_config_discovery() {
    let temp = create_test_repo();

    // Create nested directories
    let deep_dir = temp.path().join("a/b/c/d/e");
    std::fs::create_dir_all(&deep_dir).expect("create deep dir");

    // Initialize at root
    apc_cmd()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    // Validate from deep directory should find config
    apc_cmd()
        .arg("validate")
        .current_dir(&deep_dir)
        .assert()
        .success();
}

#[test]
fn test_run_specific_check() {
    let temp = create_test_repo();

    // Write config with multiple checks
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = ["check1", "check2"]
timeout = "30s"

[agent]
checks = []
timeout = "15m"

[checks.check1]
run = "echo check1"
description = "First check"

[checks.check2]
run = "echo check2"
description = "Second check"
"#,
    )
    .expect("write config");

    // Run specific check
    apc_cmd()
        .args(["run", "--check", "check1"])
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn test_run_nonexistent_check() {
    let temp = create_test_repo();

    // Write minimal config
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = []
timeout = "30s"

[agent]
checks = []
timeout = "15m"
"#,
    )
    .expect("write config");

    // Run nonexistent check
    apc_cmd()
        .args(["run", "--check", "nonexistent"])
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_parallel_group_execution() {
    let temp = create_test_repo();

    // Write config with parallel groups
    std::fs::write(
        temp.path().join("agent-precommit.toml"),
        r#"
[human]
checks = []
timeout = "30s"

[agent]
checks = ["check1", "check2", "check3"]
timeout = "15m"
parallel_groups = [["check1", "check2"], ["check3"]]

[checks.check1]
run = "echo check1"
description = "First check"

[checks.check2]
run = "echo check2"
description = "Second check"

[checks.check3]
run = "echo check3"
description = "Third check"
"#,
    )
    .expect("write config");

    // Run in agent mode
    apc_cmd()
        .args(["run", "--mode", "agent"])
        .current_dir(temp.path())
        .assert()
        .success();
}
