use assert_cmd::Command;
use std::path::Path;

/// Test that `utxray cbor diff` with identical hex inputs returns identical: true.
#[test]
fn test_cbor_diff_identical() {
    let hex = "d8798344aabbccdd1903e81a004c4b40";
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg(hex)
        .arg("--right")
        .arg(hex);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["identical"], true);
    assert!(parsed["differences"].as_array().unwrap().is_empty());
    assert_eq!(parsed["left_summary"]["kind"], "constructor");
    assert_eq!(parsed["left_summary"]["constructor"], 0);
    assert_eq!(parsed["left_summary"]["field_count"], 3);
}

/// Test that `utxray cbor diff` with different values returns differences.
#[test]
fn test_cbor_diff_different() {
    let left = "d8798344aabbccdd1903e81a004c4b40";
    let right = "d87a82d87981182a1863";
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg(left)
        .arg("--right")
        .arg(right);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["identical"], false);
    assert!(!parsed["differences"].as_array().unwrap().is_empty());
    assert_eq!(parsed["left_summary"]["constructor"], 0);
    assert_eq!(parsed["right_summary"]["constructor"], 1);
}

/// Test that `utxray cbor diff` with missing --left returns error.
#[test]
fn test_cbor_diff_missing_left() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--right")
        .arg("d8798344aabbccdd1903e81a004c4b40");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].as_str().unwrap().contains("--left"));
}

/// Test that `utxray cbor diff` with missing --right returns error.
#[test]
fn test_cbor_diff_missing_right() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg("d8798344aabbccdd1903e81a004c4b40");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].as_str().unwrap().contains("--right"));
}

/// Test that `utxray cbor diff` with no args returns error.
#[test]
fn test_cbor_diff_no_args() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor").arg("diff");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
}

/// Test that `utxray cbor diff` with invalid hex returns error.
#[test]
fn test_cbor_diff_invalid_hex() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg("zzzzzz")
        .arg("--right")
        .arg("d8798344aabbccdd1903e81a004c4b40");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].is_string());
}

/// Test that `utxray cbor diff` reads from file paths.
#[test]
fn test_cbor_diff_from_files() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cbor");
    let left_path = fixture_dir.join("datum_simple.hex");
    let right_path = fixture_dir.join("datum_nested.hex");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg(left_path.to_str().unwrap())
        .arg("--right")
        .arg(right_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["identical"], false);
    assert!(!parsed["differences"].as_array().unwrap().is_empty());
}

/// Test that differences have the expected structure.
#[test]
fn test_cbor_diff_difference_structure() {
    // Same constructor but different last field value
    let left = "d8798344aabbccdd1903e81a004c4b40";
    let right = "d8798344aabbccdd1903e81a005b8d80";

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("diff")
        .arg("--left")
        .arg(left)
        .arg("--right")
        .arg(right);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["identical"], false);

    let diffs = parsed["differences"].as_array().unwrap();
    assert!(!diffs.is_empty());
    // Each difference should have path, left, right, type
    for diff in diffs {
        assert!(diff["path"].is_string(), "diff should have path");
        assert!(diff.get("left").is_some(), "diff should have left");
        assert!(diff.get("right").is_some(), "diff should have right");
        assert!(diff["type"].is_string(), "diff should have type");
    }
}
