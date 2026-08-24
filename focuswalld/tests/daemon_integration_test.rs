use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_focuswalld_cli_status_boundary_evaluation() {
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    // Test status at 19:59:59 (BLOCKED)
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path, "--fake-now", "19:59:59"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Window Status: [Blocked]"))
        .stdout(predicate::str::contains("Currently Blocked Domains Count: 8"));

    // Test status at 20:00:00 (ALLOWED)
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path, "--fake-now", "20:00:00"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Window Status: [Allowed]"))
        .stdout(predicate::str::contains("Currently Blocked Domains Count: 0"));

    // Test status at 20:59:59 (ALLOWED)
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path, "--fake-now", "20:59:59"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Window Status: [Allowed]"));

    // Test status at 21:00:00 (BLOCKED)
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path, "--fake-now", "21:00:00"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Window Status: [Blocked]"))
        .stdout(predicate::str::contains("Currently Blocked Domains Count: 8"));
}

#[test]
fn test_focuswalld_daemon_run_once_writes_dns_config() {
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    let dns_file = NamedTempFile::new().unwrap();
    let dns_path = dns_file.path().to_str().unwrap();

    // Run daemon at 19:59:59 -> Should write blocked rules
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args([
        "--db-path", db_path,
        "--dns-conf-path", dns_path,
        "--fake-now", "19:59:59",
        "--run-once",
    ])
    .assert()
    .success();

    let dns_content = std::fs::read_to_string(dns_path).unwrap();
    assert!(dns_content.contains("address=/youtube.com/0.0.0.0"));
    assert!(dns_content.contains("address=/googlevideo.com/0.0.0.0"));

    // Run daemon at 20:30:00 -> Should write allowed / empty rules
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args([
        "--db-path", db_path,
        "--dns-conf-path", dns_path,
        "--fake-now", "20:30:00",
        "--run-once",
    ])
    .assert()
    .success();

    let dns_content_allowed = std::fs::read_to_string(dns_path).unwrap();
    assert!(dns_content_allowed.contains("# No active domains blocked at this time."));
}
