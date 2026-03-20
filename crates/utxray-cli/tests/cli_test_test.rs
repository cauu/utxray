use assert_cmd::Command;

/// Test that `utxray test` returns structured JSON error when aiken is not found.
#[test]
fn test_test_returns_json_on_aiken_not_found() {
    let mut cmd = Command::cargo_bin("utxray").expect("binary should exist");
    cmd.arg("--project")
        .arg(".")
        .arg("test")
        .env("PATH", "/nonexistent");

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["errors"].is_array());
}

/// Test that `utxray test` output includes required top-level fields.
#[test]
fn test_test_output_has_version_and_status() {
    let mut cmd = Command::cargo_bin("utxray").expect("binary should exist");
    cmd.arg("--project")
        .arg(".")
        .arg("test")
        .env("PATH", "/nonexistent");

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert!(parsed.get("v").is_some(), "output must have 'v' field");
    assert!(
        parsed.get("status").is_some(),
        "output must have 'status' field"
    );
}

/// Test that `utxray test --seed` arg is accepted.
#[test]
fn test_test_accepts_seed_arg() {
    let mut cmd = Command::cargo_bin("utxray").expect("binary should exist");
    cmd.arg("--project")
        .arg(".")
        .arg("test")
        .arg("--seed")
        .arg("42")
        .env("PATH", "/nonexistent");

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should still produce valid JSON (error because aiken not found, but structured)
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray test --match` arg is accepted.
#[test]
fn test_test_accepts_match_arg() {
    let mut cmd = Command::cargo_bin("utxray").expect("binary should exist");
    cmd.arg("--project")
        .arg(".")
        .arg("test")
        .arg("--match")
        .arg("can_unlock")
        .env("PATH", "/nonexistent");

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
}

/// Test with a real Aiken project (requires aiken installed).
#[test]
#[ignore]
fn test_test_with_aiken_project() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").expect("binary should exist");
    cmd.arg("--project")
        .arg(fixture_dir.to_str().expect("valid path"))
        .arg("test");

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    // Should be ok or mixed (not error — that means the tool itself failed)
    let status = parsed["status"].as_str().expect("status should be string");
    assert!(
        status == "ok" || status == "mixed",
        "Expected ok or mixed, got: {status}"
    );
    assert!(parsed["summary"].is_object());
    assert!(parsed["results"].is_array());
    assert!(parsed["summary"]["total"].is_number());
    assert!(parsed["summary"]["passed"].is_number());
    assert!(parsed["summary"]["failed"].is_number());
}
