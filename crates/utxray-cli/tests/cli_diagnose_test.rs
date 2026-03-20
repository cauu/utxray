use assert_cmd::Command;

/// Test that `utxray diagnose` without --from returns a structured JSON error.
#[test]
fn test_diagnose_no_from_returns_error() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("diagnose");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "INVALID_INPUT");
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test that `utxray diagnose --from <nonexistent>` returns file read error.
#[test]
fn test_diagnose_missing_file_returns_error() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("diagnose")
        .arg("--from")
        .arg("/nonexistent/file.json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "FILE_READ_ERROR");
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test that `utxray diagnose --from <fixture>` classifies correctly.
#[test]
fn test_diagnose_with_fixture_classifies() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/test_fail_result.json");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("diagnose")
        .arg("--from")
        .arg(fixture_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["v"], "0.1.0");
    // The fixture has "deadline" and "validity interval fail" traces
    assert!(parsed["error_code"].is_string());
    assert!(parsed["severity"].is_string());
    assert!(parsed["category"].is_string());
    assert!(parsed["confidence"].is_string());
    assert!(parsed["source_command"].is_string());
    assert!(parsed["matched_rules"].is_array());
    assert!(parsed["summary"].is_string());
    assert!(parsed["evidence"].is_object());
    assert!(parsed["suggested_commands"].is_array());
    assert!(parsed["related_errors"].is_array());
}
