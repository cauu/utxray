use assert_cmd::Command;

/// Test that `utxray scaffold test` on a project with a blueprint returns ok.
#[test]
fn test_scaffold_test_with_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("scaffold")
        .arg("test")
        .arg("--validator")
        .arg("escrow.escrow.spend");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}

/// Test that `utxray scaffold test` on a project without blueprint returns error.
#[test]
fn test_scaffold_test_no_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/no_blueprint");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("scaffold")
        .arg("test")
        .arg("--validator")
        .arg("some_validator");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray scaffold test --validator nonexistent` on escrow returns structured JSON.
#[test]
fn test_scaffold_test_unknown_validator() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("scaffold")
        .arg("test")
        .arg("--validator")
        .arg("nonexistent_validator_name");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
