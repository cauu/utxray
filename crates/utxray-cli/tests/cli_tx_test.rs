use assert_cmd::Command;
use std::path::Path;

fn fixture_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// Test that `utxray tx build --spec <valid>` returns status ok with summary.
#[test]
fn test_tx_build_valid_spec() {
    let spec = fixture_path("tx_spec_valid.json");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("build")
        .arg("--spec")
        .arg(&spec);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["tx_file"].is_string());
    assert!(parsed["summary"]["inputs_count"].is_number());
    assert_eq!(parsed["summary"]["inputs_count"], 2);
    assert_eq!(parsed["summary"]["outputs_count"], 2);
    assert!(parsed["summary"]["scripts_invoked"].is_array());

    let scripts = parsed["summary"]["scripts_invoked"].as_array().unwrap();
    assert!(!scripts.is_empty());

    // Check that escrow.spend and token.mint are in scripts_invoked
    let names: Vec<&str> = scripts.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        names.contains(&"escrow.spend"),
        "expected escrow.spend in scripts_invoked"
    );
    assert!(
        names.contains(&"token.mint"),
        "expected token.mint in scripts_invoked"
    );

    assert!(parsed["summary"]["total_input_lovelace"].is_number());
    assert!(parsed["summary"]["total_output_lovelace"].is_number());
    assert!(parsed["summary"]["estimated_fee"].is_number());

    // tx_file should exist on disk
    let tx_file = parsed["tx_file"].as_str().unwrap();
    assert!(
        Path::new(tx_file).exists(),
        "tx_file should exist: {tx_file}"
    );

    // Cleanup
    let _ = std::fs::remove_file(tx_file);
}

/// Test that without --include-raw, tx_cbor is NOT in the output.
#[test]
fn test_tx_build_no_include_raw_omits_tx_cbor() {
    let spec = fixture_path("tx_spec_valid.json");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("build")
        .arg("--spec")
        .arg(&spec);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert!(parsed.get("tx_cbor").is_none() || parsed["tx_cbor"].is_null());

    // Cleanup
    if let Some(tx_file) = parsed["tx_file"].as_str() {
        let _ = std::fs::remove_file(tx_file);
    }
}

/// Test that with --include-raw, tx_cbor IS present in the output.
#[test]
fn test_tx_build_include_raw_has_tx_cbor() {
    let spec = fixture_path("tx_spec_valid.json");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("--include-raw")
        .arg("tx")
        .arg("build")
        .arg("--spec")
        .arg(&spec);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["tx_cbor"].is_string(),
        "tx_cbor should be present with --include-raw"
    );

    // Cleanup
    if let Some(tx_file) = parsed["tx_file"].as_str() {
        let _ = std::fs::remove_file(tx_file);
    }
}

/// Test that an invalid/missing spec file returns status error.
#[test]
fn test_tx_build_invalid_spec_file() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("build")
        .arg("--spec")
        .arg("/nonexistent/spec.json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error_code"].is_string());
}

/// Test that missing --spec flag returns a structured error.
#[test]
fn test_tx_build_missing_spec_flag() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("tx").arg("build");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "MISSING_ARGUMENT");
}

/// Test with a malformed JSON spec file.
#[test]
fn test_tx_build_malformed_json_spec() {
    let dir = std::env::temp_dir().join("utxray_test_malformed_spec");
    std::fs::create_dir_all(&dir).unwrap();
    let spec_path = dir.join("bad_spec.json");
    std::fs::write(&spec_path, "not valid json {{{").unwrap();

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("tx")
        .arg("build")
        .arg("--spec")
        .arg(spec_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "TX_BUILD_FAILED");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}
