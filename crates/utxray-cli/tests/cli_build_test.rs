use assert_cmd::Command;

/// Test that `utxray build` returns structured JSON error when aiken is not found.
/// This test always runs because it doesn't require aiken to be installed —
/// it verifies the error handling path.
#[test]
fn test_build_returns_json_on_aiken_not_found() {
    // Run with a PATH that won't contain aiken, pointing at a valid project dir
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("build")
        .env("PATH", "/nonexistent");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should output valid JSON even when aiken is not found
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["errors"].is_array());
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test that `utxray build` with aiken available on a valid project produces
/// structured JSON with validators.
/// This test is ignored by default since it requires aiken to be installed.
#[test]
#[ignore]
fn test_build_success_with_aiken() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hello_world");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("build");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert!(parsed["validators"].is_array());
    assert!(parsed["blueprint_path"].is_string());
    assert!(parsed["compile_time_ms"].is_number());
}

/// Test that `utxray build` on a broken project returns structured errors.
/// Ignored by default since it requires aiken.
#[test]
#[ignore]
fn test_build_failure_with_broken_project() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/broken_syntax");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("build");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["errors"].is_array());
    assert!(!parsed["errors"].as_array().unwrap().is_empty());
}
