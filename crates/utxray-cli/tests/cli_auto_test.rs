use assert_cmd::Command;

/// Test that `utxray auto` on a project with a blueprint returns structured JSON.
#[test]
fn test_auto_with_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("auto")
        .env("PATH", "/nonexistent"); // avoid running aiken

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}

/// Test that `utxray auto` with --scenario full on a project without blueprint returns error.
#[test]
fn test_auto_no_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/no_blueprint");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("auto")
        .arg("--scenario")
        .arg("full");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray auto` on nonexistent project returns structured error.
#[test]
fn test_auto_nonexistent_project() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg("/nonexistent/project").arg("auto");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray auto --validator nonexistent` returns structured JSON.
#[test]
fn test_auto_invalid_validator() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("auto")
        .arg("--validator")
        .arg("nonexistent_validator")
        .env("PATH", "/nonexistent");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
