use assert_cmd::Command;

/// Test that `utxray replay bundle` without --from returns a structured JSON error.
#[test]
fn test_replay_bundle_no_from_returns_error() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("replay").arg("bundle");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "INVALID_INPUT");
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test that `utxray replay bundle --from <fixture>` creates a bundle.
#[test]
fn test_replay_bundle_creates_bundle() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/test_fail_result.json");

    let output_dir = tempfile::TempDir::new().unwrap();
    let bundle_path = output_dir.path().join("test.bundle.json");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("bundle")
        .arg("--from")
        .arg(fixture_path.to_str().unwrap())
        .arg("--output")
        .arg(bundle_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["bundle_path"].is_string());
    assert!(parsed["build_artifacts"]["aiken_version"].is_string());

    // Verify the bundle file exists and is valid
    assert!(bundle_path.exists());
    let bundle_content = std::fs::read_to_string(&bundle_path).unwrap();
    let bundle: serde_json::Value = serde_json::from_str(&bundle_content).unwrap_or_else(|_| {
        panic!("Expected valid JSON in bundle, got: {bundle_content}");
    });

    assert_eq!(bundle["v"], "0.1.0");
    assert!(bundle["created_at"].is_string());
    assert!(bundle["build_artifacts"]["aiken_version"].is_string());
    // protocol_params must be inline object (not hash)
    assert!(bundle["chain_snapshot"]["protocol_params"].is_object());
    // execution.result must be the original result
    assert!(bundle["execution"]["result"].is_object());
}

/// Test that `utxray replay run` without --bundle returns a structured JSON error.
#[test]
fn test_replay_run_no_bundle_returns_error() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("replay").arg("run");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "INVALID_INPUT");
    assert_eq!(parsed["v"], "0.1.0");
}

/// Test the full roundtrip: bundle then run.
#[test]
fn test_replay_bundle_then_run() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/test_fail_result.json");

    let output_dir = tempfile::TempDir::new().unwrap();
    let bundle_path = output_dir.path().join("roundtrip.bundle.json");

    // Step 1: Create bundle
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("bundle")
        .arg("--from")
        .arg(fixture_path.to_str().unwrap())
        .arg("--output")
        .arg(bundle_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });
    assert_eq!(parsed["status"], "ok");

    // Step 2: Run bundle
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("run")
        .arg("--bundle")
        .arg(bundle_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["environment_match"]["aiken_version"].is_object());
    assert!(parsed["environment_match"]["aiken_version"]["bundled"].is_string());
    assert!(parsed["environment_match"]["aiken_version"]["current"].is_string());
    assert!(parsed["execution"]["command"].is_string());
    assert!(parsed["traces"].is_array());
    // Should have extracted traces from the bundled test result
    let traces = parsed["traces"].as_array().unwrap();
    assert_eq!(traces.len(), 1);
    assert!(traces[0].as_str().unwrap().contains("deadline"));
}

/// Test that `utxray replay run --bundle <invalid>` returns proper error.
#[test]
fn test_replay_run_invalid_bundle() {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmpfile, b"not json at all").unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("replay")
        .arg("run")
        .arg("--bundle")
        .arg(tmpfile.path().to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["v"], "0.1.0");
}
