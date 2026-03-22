use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SchemaValidateError {
    #[error("blueprint not found: {0}")]
    BlueprintNotFound(String),

    #[error("validator not found: {0}")]
    ValidatorNotFound(String),

    #[error("invalid JSON input: {0}")]
    InvalidJson(String),

    #[error("blueprint parse error: {0}")]
    BlueprintParse(String),

    #[error("datum is required by schema but was not provided")]
    DatumRequired,

    #[error("redeemer is required but was not provided")]
    RedeemerRequired,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── Blueprint (CIP-0057) data structures ───────────────────────

#[derive(Debug, Deserialize)]
pub struct Blueprint {
    pub validators: Vec<BlueprintValidator>,
    #[serde(default)]
    pub definitions: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct BlueprintValidator {
    pub title: String,
    pub datum: Option<BlueprintParam>,
    pub redeemer: Option<BlueprintParam>,
    #[serde(default)]
    #[allow(dead_code)]
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct BlueprintParam {
    #[allow(dead_code)]
    pub title: Option<String>,
    pub schema: serde_json::Value,
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SchemaValidateOutput {
    pub purpose: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datum: Option<DatumResult>,
    pub redeemer: RedeemerResult,
}

#[derive(Debug, Serialize)]
pub struct DatumResult {
    pub valid: bool,
    pub required_by_schema: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize)]
pub struct RedeemerResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor_index: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub field: String,
    pub expected: String,
    pub got: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SchemaErrorData {
    pub error_code: String,
    pub message: String,
}

// ── Public API ─────────────────────────────────────────────────

/// Validate datum and redeemer against a CIP-0057 blueprint schema.
///
/// `project_dir` - path to project root (where plutus.json lives)
/// `validator` - validator name or index (e.g. "escrow.escrow.spend" or "0")
/// `purpose` - spend, mint, withdrawal, certificate, propose, vote (aliases: withdraw, cert)
/// `datum_json` - optional datum as JSON string or file path
/// `redeemer_json` - redeemer as JSON string or file path
pub fn validate_schema(
    project_dir: &str,
    validator: &str,
    purpose: &str,
    datum_json: Option<&str>,
    redeemer_json: &str,
) -> Result<Output<SchemaValidateOutput>, SchemaValidateError> {
    let blueprint = load_blueprint(project_dir)?;
    let val = find_validator(&blueprint, validator)?;

    // Validate datum
    let datum_result = if let Some(datum_param) = &val.datum {
        // Datum is required by schema
        match datum_json {
            Some(input) => {
                let data = parse_json_input(input)?;
                let schema = resolve_schema(&datum_param.schema, &blueprint.definitions)?;
                let errors = validate_data_against_schema(&data, &schema, &blueprint.definitions);
                Some(DatumResult {
                    valid: errors.is_empty(),
                    required_by_schema: true,
                    errors,
                })
            }
            None => Some(DatumResult {
                valid: false,
                required_by_schema: true,
                errors: vec![ValidationError {
                    field: "<root>".to_string(),
                    expected: "datum value".to_string(),
                    got: "nothing (datum not provided)".to_string(),
                    hint: Some(
                        "This validator requires a datum. Provide --datum argument.".to_string(),
                    ),
                }],
            }),
        }
    } else {
        // No datum schema in blueprint (e.g. mint purpose)
        datum_json.map(|input| {
            let data = match parse_json_input(input) {
                Ok(d) => d,
                Err(_) => {
                    return DatumResult {
                        valid: false,
                        required_by_schema: false,
                        errors: vec![ValidationError {
                            field: "<root>".to_string(),
                            expected: "no datum (not required by schema)".to_string(),
                            got: "invalid JSON".to_string(),
                            hint: None,
                        }],
                    };
                }
            };
            // Datum provided but not required - still validate it as pass-through
            let _ = data;
            DatumResult {
                valid: true,
                required_by_schema: false,
                errors: vec![],
            }
        })
    };

    // Validate redeemer
    let redeemer_data = parse_json_input(redeemer_json)?;
    let redeemer_result = match &val.redeemer {
        Some(param) => {
            let schema = resolve_schema(&param.schema, &blueprint.definitions)?;
            let errors =
                validate_data_against_schema(&redeemer_data, &schema, &blueprint.definitions);

            let (matched_type, constructor_index) = if errors.is_empty() {
                match_constructor_type(&redeemer_data, &schema, &val.title)
            } else {
                (None, None)
            };

            RedeemerResult {
                valid: errors.is_empty(),
                matched_type,
                constructor_index,
                errors,
            }
        }
        None => RedeemerResult {
            valid: true,
            matched_type: None,
            constructor_index: None,
            errors: vec![],
        },
    };

    let output_data = SchemaValidateOutput {
        purpose: purpose.to_string(),
        datum: datum_result,
        redeemer: redeemer_result,
    };

    Ok(Output::ok(output_data))
}

// ── Internal helpers ───────────────────────────────────────────

fn load_blueprint(project_dir: &str) -> Result<Blueprint, SchemaValidateError> {
    let path = Path::new(project_dir).join("plutus.json");
    if !path.exists() {
        return Err(SchemaValidateError::BlueprintNotFound(
            path.display().to_string(),
        ));
    }
    let content = std::fs::read_to_string(&path)?;
    let blueprint: Blueprint = serde_json::from_str(&content)
        .map_err(|e| SchemaValidateError::BlueprintParse(e.to_string()))?;
    Ok(blueprint)
}

fn find_validator<'a>(
    blueprint: &'a Blueprint,
    validator: &str,
) -> Result<&'a BlueprintValidator, SchemaValidateError> {
    // Try matching by index first
    if let Ok(idx) = validator.parse::<usize>() {
        if idx < blueprint.validators.len() {
            return Ok(&blueprint.validators[idx]);
        }
    }

    // Match by title (exact or suffix match)
    blueprint
        .validators
        .iter()
        .find(|v| v.title == validator || v.title.ends_with(validator))
        .ok_or_else(|| SchemaValidateError::ValidatorNotFound(validator.to_string()))
}

fn parse_json_input(input: &str) -> Result<serde_json::Value, SchemaValidateError> {
    // Try parsing as inline JSON first
    if let Ok(val) = serde_json::from_str(input) {
        return Ok(val);
    }
    // Try reading as file path
    let path = Path::new(input);
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        return serde_json::from_str(&content)
            .map_err(|e| SchemaValidateError::InvalidJson(e.to_string()));
    }
    Err(SchemaValidateError::InvalidJson(format!(
        "not valid JSON and not a readable file: {input}"
    )))
}

/// Resolve a `$ref` to the actual schema definition.
fn resolve_schema(
    schema: &serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, SchemaValidateError> {
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        // Format: "#/definitions/escrow~1EscrowDatum"
        // The ~1 is JSON Pointer encoding for /
        let def_key = ref_str
            .strip_prefix("#/definitions/")
            .ok_or_else(|| {
                SchemaValidateError::BlueprintParse(format!("unsupported $ref format: {ref_str}"))
            })?
            .replace("~1", "/");

        definitions.get(&def_key).cloned().ok_or_else(|| {
            SchemaValidateError::BlueprintParse(format!("definition not found: {def_key}"))
        })
    } else {
        Ok(schema.clone())
    }
}

/// Validate a PlutusData JSON value against a CIP-0057 schema definition.
///
/// The data is expected in Cardano JSON schema format:
///   - constructor: {"constructor": N, "fields": [...]}
///   - integer: {"int": N}
///   - bytes: {"bytes": "hex..."}
///   - list: {"list": [...]}
///   - map: {"map": [{"k": ..., "v": ...}, ...]}
fn validate_data_against_schema(
    data: &serde_json::Value,
    schema: &serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check the schema's dataType
    if let Some(data_type) = schema.get("dataType").and_then(|v| v.as_str()) {
        match data_type {
            "constructor" => {
                validate_constructor(data, schema, definitions, &mut errors);
            }
            "integer" => {
                if data.get("int").is_none() {
                    errors.push(ValidationError {
                        field: "<root>".to_string(),
                        expected: "integer ({\"int\": N})".to_string(),
                        got: describe_data_type(data),
                        hint: None,
                    });
                }
            }
            "bytes" => {
                if data.get("bytes").is_none() {
                    errors.push(ValidationError {
                        field: "<root>".to_string(),
                        expected: "bytes ({\"bytes\": \"hex...\"})".to_string(),
                        got: describe_data_type(data),
                        hint: None,
                    });
                }
            }
            "list" => {
                if data.get("list").is_none() {
                    errors.push(ValidationError {
                        field: "<root>".to_string(),
                        expected: "list ({\"list\": [...]})".to_string(),
                        got: describe_data_type(data),
                        hint: None,
                    });
                }
            }
            "map" => {
                if data.get("map").is_none() {
                    errors.push(ValidationError {
                        field: "<root>".to_string(),
                        expected: "map ({\"map\": [...]})".to_string(),
                        got: describe_data_type(data),
                        hint: None,
                    });
                }
            }
            _ => {} // Unknown dataType, skip
        }
    } else if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        // enum type: try to match one of the constructors
        validate_any_of(data, any_of, definitions, &mut errors);
    } else if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        // Resolve reference and recurse
        let def_key = ref_str
            .strip_prefix("#/definitions/")
            .unwrap_or(ref_str)
            .replace("~1", "/");
        if let Some(resolved) = definitions.get(&def_key) {
            let sub_errors = validate_data_against_schema(data, resolved, definitions);
            errors.extend(sub_errors);
        }
    }

    errors
}

fn validate_constructor(
    data: &serde_json::Value,
    schema: &serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
    errors: &mut Vec<ValidationError>,
) {
    let data_constructor = data.get("constructor").and_then(|v| v.as_u64());
    let schema_index = schema.get("index").and_then(|v| v.as_u64());

    match (data_constructor, schema_index) {
        (Some(dc), Some(si)) if dc != si => {
            errors.push(ValidationError {
                field: "<root>".to_string(),
                expected: format!("constructor index {si}"),
                got: format!("constructor index {dc}"),
                hint: None,
            });
            return; // No point checking fields if constructor is wrong
        }
        (None, _) => {
            errors.push(ValidationError {
                field: "<root>".to_string(),
                expected: "constructor ({\"constructor\": N, \"fields\": [...]})".to_string(),
                got: describe_data_type(data),
                hint: None,
            });
            return;
        }
        _ => {}
    }

    // Check fields
    let schema_fields = schema
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let data_fields = data
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if data_fields.len() != schema_fields.len() {
        errors.push(ValidationError {
            field: "<root>".to_string(),
            expected: format!("{} fields", schema_fields.len()),
            got: format!("{} fields", data_fields.len()),
            hint: None,
        });
        return;
    }

    // Validate each field
    for (i, (data_field, schema_field)) in data_fields.iter().zip(schema_fields.iter()).enumerate()
    {
        let field_name = schema_field
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("field_{i}"))
            .to_string();

        // Resolve field schema (may be a $ref)
        let field_schema = if let Some(ref_val) = schema_field.get("$ref") {
            let ref_str = ref_val.as_str().unwrap_or("");
            let def_key = ref_str
                .strip_prefix("#/definitions/")
                .unwrap_or(ref_str)
                .replace("~1", "/");
            definitions
                .get(&def_key)
                .cloned()
                .unwrap_or_else(|| schema_field.clone())
        } else {
            schema_field.clone()
        };

        let field_errors = validate_data_against_schema(data_field, &field_schema, definitions);
        for mut e in field_errors {
            if e.field == "<root>" {
                e.field = field_name.clone();
            } else {
                e.field = format!("{field_name}.{}", e.field);
            }
            errors.push(e);
        }
    }
}

fn validate_any_of(
    data: &serde_json::Value,
    variants: &[serde_json::Value],
    definitions: &serde_json::Map<String, serde_json::Value>,
    errors: &mut Vec<ValidationError>,
) {
    let data_constructor = data.get("constructor").and_then(|v| v.as_u64());

    // Try to find a matching variant
    for variant in variants {
        let variant_index = variant.get("index").and_then(|v| v.as_u64());
        if data_constructor == variant_index {
            // Found matching constructor index, validate fields
            let sub_errors = validate_data_against_schema(data, variant, definitions);
            errors.extend(sub_errors);
            return;
        }
    }

    // No matching constructor found
    let valid_indices: Vec<String> = variants
        .iter()
        .filter_map(|v| {
            v.get("index")
                .and_then(|i| i.as_u64())
                .map(|i| i.to_string())
        })
        .collect();

    let valid_names: Vec<String> = variants
        .iter()
        .filter_map(|v| {
            let title = v.get("title").and_then(|t| t.as_str())?;
            let idx = v.get("index").and_then(|i| i.as_u64())?;
            Some(format!("{title}({idx})"))
        })
        .collect();

    errors.push(ValidationError {
        field: "<root>".to_string(),
        expected: format!(
            "one of constructors: [{}]",
            if valid_names.is_empty() {
                valid_indices.join(", ")
            } else {
                valid_names.join(", ")
            }
        ),
        got: match data_constructor {
            Some(c) => format!("constructor index {c}"),
            None => describe_data_type(data),
        },
        hint: None,
    });
}

/// Describe the type of a PlutusData JSON value for error messages.
fn describe_data_type(data: &serde_json::Value) -> String {
    if data.get("constructor").is_some() {
        let idx = data
            .get("constructor")
            .and_then(|v| v.as_u64())
            .map(|i| i.to_string())
            .unwrap_or_else(|| "?".to_string());
        let field_count = data
            .get("fields")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!("constructor({idx}, {field_count} fields)")
    } else if let Some(bytes_val) = data.get("bytes").and_then(|v| v.as_str()) {
        let byte_len = bytes_val.len() / 2;
        format!("ByteArray ({byte_len} bytes)")
    } else if data.get("int").is_some() {
        "integer".to_string()
    } else if data.get("list").is_some() {
        let len = data
            .get("list")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!("list ({len} items)")
    } else if data.get("map").is_some() {
        "map".to_string()
    } else {
        format!("unknown ({data})")
    }
}

/// Try to match the constructor index in data against the schema's anyOf variants
/// and return the matched type name and constructor index.
fn match_constructor_type(
    data: &serde_json::Value,
    schema: &serde_json::Value,
    validator_title: &str,
) -> (Option<String>, Option<u64>) {
    let constructor_idx = match data.get("constructor").and_then(|v| v.as_u64()) {
        Some(idx) => idx,
        None => return (None, None),
    };

    let variants = match schema.get("anyOf").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => {
            // Single constructor type
            let title = schema
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            // Extract module prefix from validator title
            let module = validator_title.split('.').next().unwrap_or("");
            let schema_parent = schema
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(title);
            return (
                Some(format!("{module}.{schema_parent}")),
                Some(constructor_idx),
            );
        }
    };

    for variant in variants {
        let variant_idx = variant.get("index").and_then(|v| v.as_u64());
        if variant_idx == Some(constructor_idx) {
            let variant_title = variant
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let schema_title = schema
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let module = validator_title.split('.').next().unwrap_or("");
            return (
                Some(format!("{module}.{schema_title}.{variant_title}")),
                Some(constructor_idx),
            );
        }
    }

    (None, Some(constructor_idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/escrow")
    }

    fn dir_str(dir: &PathBuf) -> &str {
        dir.to_str().unwrap_or("/invalid")
    }

    #[test]
    fn test_load_blueprint() -> TestResult {
        let dir = fixture_dir();
        let bp = load_blueprint(dir_str(&dir))?;
        assert_eq!(bp.validators.len(), 2);
        assert_eq!(bp.validators[0].title, "escrow.escrow.spend");
        assert_eq!(bp.validators[1].title, "escrow.token.mint");
        Ok(())
    }

    #[test]
    fn test_find_validator_by_title() -> TestResult {
        let dir = fixture_dir();
        let bp = load_blueprint(dir_str(&dir))?;
        let v = find_validator(&bp, "escrow.escrow.spend")?;
        assert_eq!(v.title, "escrow.escrow.spend");
        Ok(())
    }

    #[test]
    fn test_find_validator_by_index() -> TestResult {
        let dir = fixture_dir();
        let bp = load_blueprint(dir_str(&dir))?;
        let v = find_validator(&bp, "0")?;
        assert_eq!(v.title, "escrow.escrow.spend");
        Ok(())
    }

    #[test]
    fn test_find_validator_by_suffix() -> TestResult {
        let dir = fixture_dir();
        let bp = load_blueprint(dir_str(&dir))?;
        let v = find_validator(&bp, "token.mint")?;
        assert_eq!(v.title, "escrow.token.mint");
        Ok(())
    }

    #[test]
    fn test_find_validator_not_found() -> TestResult {
        let dir = fixture_dir();
        let bp = load_blueprint(dir_str(&dir))?;
        assert!(find_validator(&bp, "nonexistent").is_err());
        Ok(())
    }

    #[test]
    fn test_validate_valid_datum_and_redeemer() -> TestResult {
        let dir = fixture_dir();
        let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccddaabbccddaabbccddaabbccdd"}, {"int": 1000}, {"int": 5000000}]}"#;
        let redeemer = r#"{"constructor": 0, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            Some(datum),
            redeemer,
        )?;
        let data = &output.data;
        assert_eq!(data.purpose, "spend");
        let datum_r = data.datum.as_ref().ok_or("expected datum result")?;
        assert!(datum_r.valid);
        assert!(data.redeemer.valid);
        assert!(data.redeemer.matched_type.is_some());
        assert_eq!(data.redeemer.constructor_index, Some(0));
        Ok(())
    }

    #[test]
    fn test_validate_wrong_constructor_redeemer() -> TestResult {
        let dir = fixture_dir();
        let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccddaabbccddaabbccddaabbccdd"}, {"int": 1000}, {"int": 5000000}]}"#;
        let redeemer = r#"{"constructor": 5, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            Some(datum),
            redeemer,
        )?;
        assert!(!output.data.redeemer.valid);
        assert!(!output.data.redeemer.errors.is_empty());
        Ok(())
    }

    #[test]
    fn test_validate_datum_wrong_field_count() -> TestResult {
        let dir = fixture_dir();
        let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccdd"}, {"int": 1000}]}"#;
        let redeemer = r#"{"constructor": 0, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            Some(datum),
            redeemer,
        )?;
        let datum_r = output.data.datum.as_ref().ok_or("expected datum result")?;
        assert!(!datum_r.valid);
        Ok(())
    }

    #[test]
    fn test_validate_datum_wrong_field_type() -> TestResult {
        let dir = fixture_dir();
        let datum =
            r#"{"constructor": 0, "fields": [{"int": 42}, {"int": 1000}, {"int": 5000000}]}"#;
        let redeemer = r#"{"constructor": 0, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            Some(datum),
            redeemer,
        )?;
        let datum_r = output.data.datum.as_ref().ok_or("expected datum result")?;
        assert!(!datum_r.valid);
        assert!(datum_r.errors.iter().any(|e| e.field == "owner"));
        Ok(())
    }

    #[test]
    fn test_validate_mint_no_datum() -> TestResult {
        let dir = fixture_dir();
        let redeemer = r#"{"constructor": 0, "fields": []}"#;
        let output = validate_schema(dir_str(&dir), "escrow.token.mint", "mint", None, redeemer)?;
        assert!(output.data.datum.is_none());
        assert!(output.data.redeemer.valid);
        Ok(())
    }

    #[test]
    fn test_validate_spend_no_datum_when_required() -> TestResult {
        let dir = fixture_dir();
        let redeemer = r#"{"constructor": 0, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            None,
            redeemer,
        )?;
        let datum_r = output.data.datum.as_ref().ok_or("expected datum result")?;
        assert!(!datum_r.valid);
        assert!(datum_r.required_by_schema);
        assert!(!datum_r.errors.is_empty());
        Ok(())
    }

    #[test]
    fn test_validate_blueprint_not_found() {
        let result = validate_schema(
            "/nonexistent/path",
            "validator",
            "spend",
            None,
            r#"{"constructor": 0, "fields": []}"#,
        );
        assert!(matches!(
            result,
            Err(SchemaValidateError::BlueprintNotFound(_))
        ));
    }

    #[test]
    fn test_validate_validator_not_found() {
        let dir = fixture_dir();
        let result = validate_schema(
            dir_str(&dir),
            "nonexistent.validator",
            "spend",
            None,
            r#"{"constructor": 0, "fields": []}"#,
        );
        assert!(matches!(
            result,
            Err(SchemaValidateError::ValidatorNotFound(_))
        ));
    }

    #[test]
    fn test_validate_invalid_redeemer_json() {
        let dir = fixture_dir();
        let result = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            None,
            "not valid json at all",
        );
        assert!(matches!(result, Err(SchemaValidateError::InvalidJson(_))));
    }

    #[test]
    fn test_redeemer_cancel_variant() -> TestResult {
        let dir = fixture_dir();
        let datum = r#"{"constructor": 0, "fields": [{"bytes": "aabbccddaabbccddaabbccddaabbccdd"}, {"int": 1000}, {"int": 5000000}]}"#;
        let redeemer = r#"{"constructor": 1, "fields": []}"#;
        let output = validate_schema(
            dir_str(&dir),
            "escrow.escrow.spend",
            "spend",
            Some(datum),
            redeemer,
        )?;
        assert!(output.data.redeemer.valid);
        assert_eq!(output.data.redeemer.constructor_index, Some(1));
        let matched = output
            .data
            .redeemer
            .matched_type
            .as_ref()
            .ok_or("expected matched_type")?;
        assert!(
            matched.contains("Cancel"),
            "Expected Cancel in matched_type, got: {matched}"
        );
        Ok(())
    }

    #[test]
    fn test_describe_data_type() {
        assert_eq!(
            describe_data_type(&serde_json::json!({"bytes": "aabb"})),
            "ByteArray (2 bytes)"
        );
        assert_eq!(
            describe_data_type(&serde_json::json!({"int": 42})),
            "integer"
        );
        assert_eq!(
            describe_data_type(&serde_json::json!({"list": [1, 2]})),
            "list (2 items)"
        );
    }
}
