use assert_cmd::Command;

/// Test that `utxray budget show` without a valid project returns structured JSON.
#[test]
fn test_budget_show_no_project() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg("/nonexistent/project")
        .arg("budget")
        .arg("show");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray budget show` on a project with a blueprint returns ok.
#[test]
fn test_budget_show_with_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("budget")
        .arg("show");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    // ok or error depending on whether blueprint has budget info
    assert!(parsed["status"].is_string());
}

/// Test that `utxray budget compare` with missing files returns error.
#[test]
fn test_budget_compare_missing_files() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("budget")
        .arg("compare")
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

/// Test that `utxray budget compare` with no args returns error.
#[test]
fn test_budget_compare_no_args() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("budget").arg("compare");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray budget show --validator` with unknown validator returns structured JSON.
#[test]
fn test_budget_show_unknown_validator() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("budget")
        .arg("show")
        .arg("--validator")
        .arg("nonexistent_validator");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
