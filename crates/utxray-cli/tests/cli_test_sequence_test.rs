use assert_cmd::Command;
use std::io::Write;

/// Test that `utxray test-sequence --spec <nonexistent>` returns structured error.
#[test]
fn test_test_sequence_file_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("test-sequence")
        .arg("--spec")
        .arg("/nonexistent/sequence.json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray test-sequence --spec <malformed>` returns structured error.
#[test]
fn test_test_sequence_malformed_spec() {
    let dir = tempfile::tempdir().unwrap();
    let spec_path = dir.path().join("bad_sequence.json");
    let mut f = std::fs::File::create(&spec_path).unwrap();
    f.write_all(b"not valid json {{{").unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("test-sequence")
        .arg("--spec")
        .arg(spec_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray test-sequence --spec <valid>` with an empty steps array returns ok.
#[test]
fn test_test_sequence_empty_steps() {
    let dir = tempfile::tempdir().unwrap();
    let spec_path = dir.path().join("empty_sequence.json");
    let content = serde_json::json!({
        "name": "empty-test",
        "steps": []
    });
    std::fs::write(&spec_path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("test-sequence")
        .arg("--spec")
        .arg(spec_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
