use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_focuswalld_cli_status_and_quota() {
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    // 1. Initial status: YouTube is Blocked by default and Adult Content is strictly blocked 24/7
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Daily 1-Hour Quota: 0m 0s used / 60m total"))
        .stdout(predicate::str::contains("YouTube Access Status: [Blocked] (Session Active: false"))
        .stdout(predicate::str::contains("Active Policies Count: 2"));

    // 2. Unlock a 30-minute session for YouTube
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["unlock-session", "--db-path", db_path, "--minutes", "30"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Session Unlocked!"))
        .stdout(predicate::str::contains("Session target: 30 minutes"));

    // 3. Status during active session -> YouTube is Allowed, Adult Content remains Blocked
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Access Status: [Allowed] (Session Active: true"));

    // 4. Lock / pause session
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["lock-session", "--db-path", db_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Session Paused / Locked."));

    // 5. Status after locking -> YouTube must be Blocked again
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["status", "--db-path", db_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("YouTube Access Status: [Blocked] (Session Active: false"));
}

#[test]
fn test_focuswalld_daemon_run_once_writes_dns_config() {
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    let dns_file = NamedTempFile::new().unwrap();
    let dns_path = dns_file.path().to_str().unwrap();

    // 1. Run daemon initially -> Should write blocked rules for YouTube and Adult Content
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args([
        "--db-path", db_path,
        "--dns-conf-path", dns_path,
        "--run-once",
    ])
    .assert()
    .success();

    let dns_content = std::fs::read_to_string(dns_path).unwrap();
    assert!(dns_content.contains("address=/youtube.com/0.0.0.0"));
    assert!(dns_content.contains("address=/googlevideo.com/0.0.0.0"));
    assert!(dns_content.contains("address=/pornhub.com/0.0.0.0"));
    assert!(dns_content.contains("address=/xvideos.com/0.0.0.0"));

    // 2. Start an unlock session for YouTube
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args(["unlock-session", "--db-path", db_path, "--minutes", "60"])
        .assert()
        .success();

    // 3. Run daemon while session is active -> YouTube is unblocked, Adult Content remains blocked!
    let mut cmd = Command::cargo_bin("focuswalld").unwrap();
    cmd.args([
        "--db-path", db_path,
        "--dns-conf-path", dns_path,
        "--run-once",
    ])
    .assert()
    .success();

    let dns_content_allowed = std::fs::read_to_string(dns_path).unwrap();
    assert!(!dns_content_allowed.contains("address=/youtube.com/0.0.0.0"));
    assert!(dns_content_allowed.contains("address=/pornhub.com/0.0.0.0"));
    assert!(dns_content_allowed.contains("address=/xvideos.com/0.0.0.0"));
}
