use assert_cmd::Command;

/// Test that `utxray cbor decode --hex` with a valid simple datum returns correct JSON.
#[test]
fn test_cbor_decode_simple_datum() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("decode")
        .arg("--hex")
        .arg("d8798344aabbccdd1903e81a004c4b40");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["decoded"]["constructor"], 0);

    let fields = parsed["decoded"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0]["bytes"], "aabbccdd");
    assert_eq!(fields[1]["int"], 1000);
    assert_eq!(fields[2]["int"], 5000000);
}

/// Test that `utxray cbor decode --hex` with a nested datum returns correct JSON.
#[test]
fn test_cbor_decode_nested_datum() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("decode")
        .arg("--hex")
        .arg("d87a82d87981182a1863");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["decoded"]["constructor"], 1);

    let fields = parsed["decoded"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["constructor"], 0);
    assert_eq!(fields[0]["fields"][0]["int"], 42);
    assert_eq!(fields[1]["int"], 99);
}

/// Test that `utxray cbor decode --hex` with invalid hex returns structured error JSON.
#[test]
fn test_cbor_decode_invalid_hex() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor").arg("decode").arg("--hex").arg("zzzzzz");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["v"], "0.1.0");
    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].is_string());
}

/// Test that `utxray cbor decode` with no --hex or --file returns error JSON.
#[test]
fn test_cbor_decode_no_input() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor").arg("decode");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].as_str().unwrap().contains("--hex"));
}

/// Test that `utxray cbor decode --file` reads from a fixture file.
#[test]
fn test_cbor_decode_from_file() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cbor/datum_simple.hex");

    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor")
        .arg("decode")
        .arg("--file")
        .arg(fixture_path.to_str().unwrap());

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["decoded"]["constructor"], 0);
}

/// Test that `utxray cbor decode --hex` with valid hex but invalid CBOR returns error.
#[test]
fn test_cbor_decode_invalid_cbor() {
    let mut cmd = Command::cargo_bin("utxray").unwrap();
    cmd.arg("cbor").arg("decode").arg("--hex").arg("ff");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("Expected valid JSON output, got: {stdout}");
    });

    assert_eq!(parsed["status"], "error");
    assert!(parsed["error"].is_string());
}
