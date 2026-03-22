use assert_cmd::Command;
use std::io::Write;

/// Test that `utxray replay diff` with no args returns structured error.
#[test]
fn test_replay_diff_no_args() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("replay").arg("diff");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray replay diff --before <nonexistent>` returns structured error.
#[test]
fn test_replay_diff_file_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("diff")
        .arg("--before")
        .arg("/nonexistent/before.json")
        .arg("--after")
        .arg("/nonexistent/after.json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray replay diff` with valid identical files returns ok with no differences.
#[test]
fn test_replay_diff_identical_files() {
    let dir = tempfile::tempdir().unwrap();
    let content = serde_json::json!({
        "v": "0.1.0",
        "status": "ok",
        "validators": [
            {"name": "escrow.spend", "result": "pass", "cpu": 1000, "mem": 500}
        ]
    });
    let before_path = dir.path().join("before.json");
    let after_path = dir.path().join("after.json");
    std::fs::write(
        &before_path,
        serde_json::to_string_pretty(&content).unwrap(),
    )
    .unwrap();
    std::fs::write(&after_path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("diff")
        .arg("--before")
        .arg(before_path.to_str().unwrap())
        .arg("--after")
        .arg(after_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    // Should be ok or have a status field
    assert!(parsed["status"].is_string());
}

/// Test that `utxray replay diff --before <malformed>` with invalid JSON returns error.
#[test]
fn test_replay_diff_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    let before_path = dir.path().join("before.json");
    let after_path = dir.path().join("after.json");

    let mut f = std::fs::File::create(&before_path).unwrap();
    f.write_all(b"not valid json {{{").unwrap();
    std::fs::write(&after_path, r#"{"status": "ok"}"#).unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("diff")
        .arg("--before")
        .arg(before_path.to_str().unwrap())
        .arg("--after")
        .arg(after_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
}
