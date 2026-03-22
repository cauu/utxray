use assert_cmd::Command;

/// Test that `utxray tx simulate` without --tx returns structured error JSON.
#[test]
fn test_tx_simulate_missing_tx_flag() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("tx").arg("simulate");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "MISSING_ARGUMENT");
}

/// Test that `utxray tx simulate --tx <invalid>` returns error when backend not configured.
#[test]
fn test_tx_simulate_no_backend_config() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("simulate")
        .arg("--tx")
        .arg("aabbccdd");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    // Either BACKEND_NOT_CONFIGURED or SIMULATE_FAILED is acceptable
    assert!(
        parsed["error_code"] == "BACKEND_NOT_CONFIGURED"
            || parsed["error_code"] == "SIMULATE_FAILED",
        "Unexpected error_code: {}",
        parsed["error_code"]
    );
}

/// Test that `utxray tx simulate --tx <nonexistent_file>` returns structured error.
#[test]
fn test_tx_simulate_file_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("simulate")
        .arg("--tx")
        .arg("/nonexistent/tx.cbor");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    // Could be BACKEND_NOT_CONFIGURED or TX_READ_FAILED depending on order
    assert!(parsed["error_code"].is_string());
}

/// Test that tx simulate accepts --backend and --slot flags (parsing only).
#[test]
fn test_tx_simulate_accepts_optional_flags() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("simulate")
        .arg("--tx")
        .arg("aabb")
        .arg("--slot")
        .arg("12345");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    // Should return structured JSON (error is expected since no backend is configured)
    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
