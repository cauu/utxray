use assert_cmd::Command;
use std::path::Path;

fn fixture_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// Copy the tx spec fixture into a unique temp dir so tests don't race on the output file.
fn setup_tx_spec() -> (std::path::PathBuf, String) {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("utxray_tx_test_{id}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    let spec_src = fixture_path("tx_spec_valid.json");
    let spec_dst = dir.join("tx_spec.json");
    std::fs::copy(&spec_src, &spec_dst).ok();
    let spec_path = spec_dst.to_string_lossy().into_owned();
    (dir, spec_path)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_tx_build_valid_spec() {
    let (dir, spec) = setup_tx_spec();

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
    assert_eq!(parsed["summary"]["inputs_count"], 2);
    assert_eq!(parsed["summary"]["outputs_count"], 2);
    assert!(parsed["summary"]["scripts_invoked"].is_array());

    let scripts = parsed["summary"]["scripts_invoked"].as_array().unwrap();
    assert!(!scripts.is_empty());

    let names: Vec<&str> = scripts.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"escrow.spend"));
    assert!(names.contains(&"token.mint"));

    assert!(parsed["summary"]["total_input_lovelace"].is_number());
    assert!(parsed["summary"]["total_output_lovelace"].is_number());
    assert!(parsed["summary"]["estimated_fee"].is_number());

    if let Some(tx_file) = parsed["tx_file"].as_str() {
        assert!(Path::new(tx_file).exists(), "tx_file should exist: {tx_file}");
    }

    cleanup(&dir);
}

#[test]
fn test_tx_build_no_include_raw_omits_tx_cbor() {
    let (dir, spec) = setup_tx_spec();

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

    cleanup(&dir);
}

#[test]
fn test_tx_build_include_raw_has_tx_cbor() {
    let (dir, spec) = setup_tx_spec();

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

    cleanup(&dir);
}

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

#[test]
fn test_tx_build_malformed_json_spec() {
    let dir = std::env::temp_dir().join(format!(
        "utxray_test_malformed_{}",
        std::process::id()
    ));
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

    let _ = std::fs::remove_dir_all(&dir);
}
