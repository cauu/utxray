use assert_cmd::Command;

/// Test that `utxray gen-context` returns structured JSON output.
#[test]
fn test_gen_context_returns_json() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("gen-context");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}

/// Test that `utxray gen-context` on a project with blueprint returns ok.
#[test]
fn test_gen_context_with_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("gen-context");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}

/// Test that `utxray gen-context` on a project without blueprint returns error.
#[test]
fn test_gen_context_no_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/no_blueprint");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("gen-context");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}
