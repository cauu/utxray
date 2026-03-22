use assert_cmd::Command;

/// Test that `utxray utxo diff` without --address returns structured error.
#[test]
fn test_utxo_diff_missing_address() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("utxo")
        .arg("diff")
        .arg("--before-tx")
        .arg("abc123")
        .arg("--after-tx")
        .arg("def456");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "MISSING_ARGUMENT");
}

/// Test that `utxo diff` without mode args (no --before-tx/--after-tx or --before-slot/--after-slot)
/// returns structured error.
#[test]
fn test_utxo_diff_missing_mode_args() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("utxo")
        .arg("diff")
        .arg("--address")
        .arg("addr_test1abc");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    // Should get either BACKEND_NOT_CONFIGURED (if no blockfrost config) or MISSING_ARGUMENT
    assert!(
        parsed["error_code"] == "BACKEND_NOT_CONFIGURED"
            || parsed["error_code"] == "MISSING_ARGUMENT",
        "Unexpected error_code: {}",
        parsed["error_code"]
    );
}

/// Test that `utxo diff` with address and before-tx/after-tx returns error when backend not configured.
#[test]
fn test_utxo_diff_no_backend_config() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("utxo")
        .arg("diff")
        .arg("--address")
        .arg("addr_test1abc")
        .arg("--before-tx")
        .arg("abc123")
        .arg("--after-tx")
        .arg("def456");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "BACKEND_NOT_CONFIGURED");
}

/// Test that `utxo diff` with slot mode also returns error when backend not configured.
#[test]
fn test_utxo_diff_slot_mode_no_backend() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("utxo")
        .arg("diff")
        .arg("--address")
        .arg("addr_test1abc")
        .arg("--before-slot")
        .arg("1000")
        .arg("--after-slot")
        .arg("2000");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "BACKEND_NOT_CONFIGURED");
}

/// Test that the command accepts all expected arguments without crashing.
#[test]
fn test_utxo_diff_accepts_all_flags() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("utxo")
        .arg("diff")
        .arg("--address")
        .arg("addr_test1abc")
        .arg("--before-tx")
        .arg("abc")
        .arg("--after-tx")
        .arg("def");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should produce valid JSON regardless of error
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
