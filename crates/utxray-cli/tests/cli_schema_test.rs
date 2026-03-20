use assert_cmd::Command;

fn escrow_fixture_dir() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow");
    path.to_str().unwrap().to_string()
}

/// Test: valid datum + valid redeemer -> both valid:true, status:"ok"
#[test]
fn test_schema_validate_valid_datum_and_redeemer() {
    let project = escrow_fixture_dir();
    let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccddaabbccddaabbccddaabbccdd"}, {"int": 1000}, {"int": 5000000}]}"#;
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("escrow.escrow.spend")
        .arg("--purpose")
        .arg("spend")
        .arg("--datum")
        .arg(datum)
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["purpose"], "spend");
    assert_eq!(parsed["datum"]["valid"], true);
    assert_eq!(parsed["datum"]["required_by_schema"], true);
    assert_eq!(parsed["redeemer"]["valid"], true);
    assert!(parsed["redeemer"]["matched_type"].is_string());
    assert_eq!(parsed["redeemer"]["constructor_index"], 0);
}

/// Test: invalid datum (wrong field type) -> datum.valid:false, errors non-empty
#[test]
fn test_schema_validate_invalid_datum_wrong_type() {
    let project = escrow_fixture_dir();
    // owner should be bytes, not int
    let datum = r#"{"constructor": 0, "fields": [{"int": 42}, {"int": 1000}, {"int": 5000000}]}"#;
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("escrow.escrow.spend")
        .arg("--purpose")
        .arg("spend")
        .arg("--datum")
        .arg(datum)
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(
        parsed["status"], "ok",
        "status should be ok even with invalid datum"
    );
    assert_eq!(parsed["datum"]["valid"], false);
    let errors = parsed["datum"]["errors"].as_array().unwrap();
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(errors.iter().any(|e| e["field"] == "owner"));
}

/// Test: mint purpose with no datum -> no datum section (or datum not required)
#[test]
fn test_schema_validate_mint_no_datum() {
    let project = escrow_fixture_dir();
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("escrow.token.mint")
        .arg("--purpose")
        .arg("mint")
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["purpose"], "mint");
    // For mint, datum should not be present in output
    assert!(
        parsed.get("datum").is_none() || parsed["datum"].is_null(),
        "Mint validator should not have datum section, got: {}",
        parsed["datum"]
    );
    assert_eq!(parsed["redeemer"]["valid"], true);
}

/// Test: validator not found -> status:"error"
#[test]
fn test_schema_validate_validator_not_found() {
    let project = escrow_fixture_dir();
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("nonexistent")
        .arg("--purpose")
        .arg("spend")
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "VALIDATOR_NOT_FOUND");
}

/// Test: blueprint not found -> status:"error"
#[test]
fn test_schema_validate_blueprint_not_found() {
    // Use a real directory that exists but has no plutus.json
    let project =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/no_blueprint");
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(project.to_str().unwrap())
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("test")
        .arg("--purpose")
        .arg("spend")
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_code"], "BLUEPRINT_NOT_FOUND");
}

/// Test: spend without datum when required -> datum.valid:false with error
#[test]
fn test_schema_validate_spend_missing_datum() {
    let project = escrow_fixture_dir();
    let redeemer = r#"{"constructor": 0, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("escrow.escrow.spend")
        .arg("--purpose")
        .arg("spend")
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["datum"]["valid"], false);
    assert_eq!(parsed["datum"]["required_by_schema"], true);
}

/// Test: redeemer Cancel variant (constructor 1) -> valid:true with matched_type containing Cancel
#[test]
fn test_schema_validate_redeemer_cancel() {
    let project = escrow_fixture_dir();
    let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccddaabbccddaabbccddaabbccdd"}, {"int": 1000}, {"int": 5000000}]}"#;
    let redeemer = r#"{"constructor": 1, "fields": []}"#;

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("--project")
        .arg(&project)
        .arg("schema")
        .arg("validate")
        .arg("--validator")
        .arg("escrow.escrow.spend")
        .arg("--purpose")
        .arg("spend")
        .arg("--datum")
        .arg(datum)
        .arg("--redeemer")
        .arg(redeemer);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["redeemer"]["valid"], true);
    assert_eq!(parsed["redeemer"]["constructor_index"], 1);
    let matched_type = parsed["redeemer"]["matched_type"].as_str().unwrap();
    assert!(
        matched_type.contains("Cancel"),
        "Expected Cancel in matched_type, got: {matched_type}"
    );
}
