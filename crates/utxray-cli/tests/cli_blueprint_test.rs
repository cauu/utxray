use assert_cmd::Command;

/// Test that `utxray blueprint show` on a project with a blueprint returns ok.
#[test]
fn test_blueprint_show_success() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("blueprint")
        .arg("show");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
}

/// Test that `utxray blueprint show` on a project without blueprint returns error.
#[test]
fn test_blueprint_show_no_blueprint() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/no_blueprint");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("blueprint")
        .arg("show");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray blueprint show --file <nonexistent>` returns error.
#[test]
fn test_blueprint_show_file_not_found() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(".")
        .arg("blueprint")
        .arg("show")
        .arg("--file")
        .arg("/nonexistent/plutus.json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray blueprint apply` without --validator returns error.
#[test]
fn test_blueprint_apply_missing_validator() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project").arg(".").arg("blueprint").arg("apply");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray blueprint convert` on escrow fixture returns structured JSON.
#[test]
fn test_blueprint_convert_success() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(fixture_dir.to_str().unwrap())
        .arg("blueprint")
        .arg("convert");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert!(parsed["status"].is_string());
}
