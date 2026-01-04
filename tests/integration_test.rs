//! Integration tests for agent-precommit CLI.

use assert_cmd::Command;
use predicates::prelude::*;
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

#[test]
fn test_help() {
    Command::cargo_bin("apc")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Smart pre-commit hooks"));
}

#[test]
fn test_version() {
    Command::cargo_bin("apc")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_detect_default_human() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
        .arg("detect")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("human"));
}

#[test]
fn test_detect_agent_mode() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
        .arg("detect")
        .env("AGENT_MODE", "1")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("agent"));
}

#[test]
fn test_init_creates_config() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
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

    Command::cargo_bin("apc")
        .unwrap()
        .args(["init", "--preset", "rust"])
        .current_dir(temp.path())
        .assert()
        .success();

    let config = std::fs::read_to_string(temp.path().join("agent-precommit.toml"))
        .expect("read config");

    assert!(config.contains("clippy"));
}

#[test]
fn test_init_already_exists() {
    let temp = create_test_repo();
    std::fs::write(temp.path().join("agent-precommit.toml"), "").expect("create config");

    Command::cargo_bin("apc")
        .unwrap()
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

    Command::cargo_bin("apc")
        .unwrap()
        .args(["init", "--force"])
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn test_validate_no_config() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
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
    Command::cargo_bin("apc")
        .unwrap()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    Command::cargo_bin("apc")
        .unwrap()
        .arg("validate")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("valid"));
}

#[test]
fn test_list_checks() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("init");

    Command::cargo_bin("apc")
        .unwrap()
        .arg("list")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Human mode checks"))
        .stderr(predicate::str::contains("Agent mode checks"));
}

#[test]
fn test_install_hook() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
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
fn test_uninstall_hook() {
    let temp = create_test_repo();

    // Install first
    Command::cargo_bin("apc")
        .unwrap()
        .arg("install")
        .current_dir(temp.path())
        .output()
        .expect("install");

    // Then uninstall
    Command::cargo_bin("apc")
        .unwrap()
        .arg("uninstall")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed"));

    assert!(!temp.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn test_skip_with_env_var() {
    let temp = create_test_repo();

    Command::cargo_bin("apc")
        .unwrap()
        .arg("run")
        .env("APC_SKIP", "1")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping"));
}

#[test]
fn test_not_git_repo() {
    let temp = TempDir::new().expect("create temp dir");

    Command::cargo_bin("apc")
        .unwrap()
        .arg("detect")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a Git repository"));
}
