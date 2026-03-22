use assert_cmd::Command;
use std::io::Write;

/// Test that `utxray uplc eval` with a nonexistent file returns structured error.
#[test]
fn test_uplc_eval_file_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("uplc")
        .arg("eval")
        .arg("/nonexistent/program.uplc");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray uplc eval` with a malformed UPLC file returns structured error.
#[test]
fn test_uplc_eval_malformed_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("bad.uplc");
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.write_all(b"this is not valid uplc content }{{}").unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("uplc")
        .arg("eval")
        .arg(file_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    // uplc eval may return "error" or "mixed" depending on internal handling
    assert!(
        parsed["status"] == "error" || parsed["status"] == "mixed",
        "Expected error or mixed status, got: {}",
        parsed["status"]
    );
}

/// Test that `utxray uplc eval` with --args invalid JSON returns structured error.
#[test]
fn test_uplc_eval_invalid_args() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("dummy.uplc");
    std::fs::write(&file_path, "(program 1.0.0 (con integer 42))").unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("uplc")
        .arg("eval")
        .arg(file_path.to_str().unwrap())
        .arg("--args")
        .arg("not valid json {{{");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(
        parsed["status"] == "error" || parsed["status"] == "mixed",
        "Expected error or mixed status, got: {}",
        parsed["status"]
    );
}
