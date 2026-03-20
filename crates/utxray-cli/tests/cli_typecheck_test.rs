use assert_cmd::Command;

/// Test that `utxray typecheck` returns structured JSON error when aiken is not found.
/// This test always runs because it doesn't require aiken to be installed.
#[test]
fn test_typecheck_returns_json_on_aiken_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("typecheck")
        .env("PATH", "/nonexistent");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["errors"].is_array());
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test that `utxray typecheck` with aiken available produces structured JSON.
/// Ignored by default since it requires aiken.
#[test]
#[ignore]
fn test_typecheck_success_with_aiken() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hello_world");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("typecheck");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert!(parsed["checked_modules"].is_number());
    assert!(parsed["warnings"].is_array());
}

/// Test typecheck with module filter.
/// Ignored by default since it requires aiken.
#[test]
#[ignore]
fn test_typecheck_with_module_filter() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hello_world");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("typecheck")
        .arg("--module")
        .arg("hello");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    // Should be either ok or error, but always valid JSON
    assert!(parsed["v"] == "0.1.0");
}
