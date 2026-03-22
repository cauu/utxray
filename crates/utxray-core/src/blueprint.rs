use std::path::Path;

use serde::Serialize;

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BlueprintError {
    #[error("blueprint not found: {0}")]
    BlueprintNotFound(String),

    #[error("blueprint parse error: {0}")]
    BlueprintParse(String),

    #[error("validator not found: {0}")]
    ValidatorNotFound(String),

    #[error("validator has no parameters to apply")]
    NotParameterized,

    #[error("--params is required for blueprint apply")]
    ParamsRequired,

    #[error("--validator is required")]
    ValidatorRequired,

    #[error("invalid params JSON: {0}")]
    InvalidParams(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── CIP-0057 data structures (full) ─────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct FullBlueprint {
    pub preamble: Preamble,
    pub validators: Vec<FullValidator>,
    #[serde(default)]
    pub definitions: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, Serialize, Clone)]
pub struct Preamble {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "plutusVersion")]
    pub plutus_version: String,
    #[serde(default)]
    pub compiler: serde_json::Value,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct FullValidator {
    pub title: String,
    #[serde(default)]
    pub datum: Option<ParamSchema>,
    #[serde(default)]
    pub redeemer: Option<ParamSchema>,
    #[serde(default, rename = "compiledCode")]
    pub compiled_code: String,
    #[serde(default)]
    pub hash: String,
    /// CIP-0057 parameters field (for parameterized validators)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ParamSchema {
    #[serde(default)]
    pub title: Option<String>,
    pub schema: serde_json::Value,
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BlueprintShowOutput {
    pub preamble: PreambleOutput,
    pub validators: Vec<ValidatorOverview>,
}

#[derive(Debug, Serialize)]
pub struct PreambleOutput {
    pub title: String,
    pub version: String,
    pub plutus_version: String,
    pub compiler: String,
}

#[derive(Debug, Serialize)]
pub struct ValidatorOverview {
    pub name: String,
    pub purpose: String,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datum_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemer_schema: Option<serde_json::Value>,
    pub parameterized: bool,
}

#[derive(Debug, Serialize)]
pub struct BlueprintApplyOutput {
    pub validator: String,
    pub hash: String,
    pub params_applied: serde_json::Value,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct BlueprintConvertOutput {
    pub validator: String,
    pub text_envelope: TextEnvelope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written_to: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TextEnvelope {
    #[serde(rename = "type")]
    pub type_field: String,
    pub description: String,
    #[serde(rename = "cborHex")]
    pub cbor_hex: String,
}

#[derive(Debug, Serialize)]
pub struct BlueprintErrorData {
    pub error_code: String,
    pub message: String,
}

// ── Public API ─────────────────────────────────────────────────

/// Show blueprint overview.
pub fn blueprint_show(
    project_dir: &str,
    file: Option<&str>,
) -> Result<Output<BlueprintShowOutput>, BlueprintError> {
    let bp = load_full_blueprint(project_dir, file)?;

    let compiler_str = match &bp.preamble.compiler {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let version = obj
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("{name} {version}")
        }
        _ => "unknown".to_string(),
    };

    let preamble = PreambleOutput {
        title: bp.preamble.title.clone(),
        version: bp.preamble.version.clone(),
        plutus_version: bp.preamble.plutus_version.clone(),
        compiler: compiler_str,
    };

    let validators = bp
        .validators
        .iter()
        .map(|v| {
            let (name, purpose) = parse_validator_title(&v.title);
            ValidatorOverview {
                name,
                purpose,
                hash: v.hash.clone(),
                datum_schema: v.datum.as_ref().map(|d| d.schema.clone()),
                redeemer_schema: v.redeemer.as_ref().map(|r| r.schema.clone()),
                parameterized: v.parameters.is_some(),
            }
        })
        .collect();

    Ok(Output::ok(BlueprintShowOutput {
        preamble,
        validators,
    }))
}

/// Apply parameters to a parameterized validator (v1: validation only, no UPLC application).
pub fn blueprint_apply(
    project_dir: &str,
    file: Option<&str>,
    validator_name: Option<&str>,
    params_json: Option<&str>,
) -> Result<Output<BlueprintApplyOutput>, BlueprintError> {
    let validator_name = validator_name.ok_or(BlueprintError::ValidatorRequired)?;
    let params_str = params_json.ok_or(BlueprintError::ParamsRequired)?;

    let params: serde_json::Value = serde_json::from_str(params_str)
        .map_err(|e| BlueprintError::InvalidParams(e.to_string()))?;

    let bp = load_full_blueprint(project_dir, file)?;
    let val = find_validator(&bp, validator_name)?;

    if val.parameters.is_none() {
        return Err(BlueprintError::NotParameterized);
    }

    Ok(Output::ok(BlueprintApplyOutput {
        validator: val.title.clone(),
        hash: val.hash.clone(),
        params_applied: params,
        note: "v1: parameter application recorded but UPLC application deferred. Hash unchanged."
            .to_string(),
    }))
}

/// Convert a blueprint validator to cardano-cli text envelope format.
pub fn blueprint_convert(
    project_dir: &str,
    file: Option<&str>,
    validator_name: Option<&str>,
    out_file: Option<&str>,
) -> Result<Output<BlueprintConvertOutput>, BlueprintError> {
    let validator_name = validator_name.ok_or(BlueprintError::ValidatorRequired)?;

    let bp = load_full_blueprint(project_dir, file)?;
    let val = find_validator(&bp, validator_name)?;

    let plutus_version = &bp.preamble.plutus_version;
    let type_field = match plutus_version.as_str() {
        "v1" => "PlutusScriptV1",
        "v2" => "PlutusScriptV2",
        _ => "PlutusScriptV3",
    };

    // The compiledCode in the blueprint is already the double-CBOR-encoded script hex.
    // For cardano-cli text envelope, we wrap it as-is in the cborHex field.
    let envelope = TextEnvelope {
        type_field: type_field.to_string(),
        description: String::new(),
        cbor_hex: val.compiled_code.clone(),
    };

    let written_to = if let Some(path) = out_file {
        let envelope_json = serde_json::to_string_pretty(&envelope)
            .map_err(|e| BlueprintError::BlueprintParse(e.to_string()))?;
        std::fs::write(path, &envelope_json)?;
        Some(path.to_string())
    } else {
        None
    };

    Ok(Output::ok(BlueprintConvertOutput {
        validator: val.title.clone(),
        text_envelope: envelope,
        written_to,
    }))
}

// ── Internal helpers ───────────────────────────────────────────

fn load_full_blueprint(
    project_dir: &str,
    file: Option<&str>,
) -> Result<FullBlueprint, BlueprintError> {
    let path = match file {
        Some(f) => Path::new(f).to_path_buf(),
        None => Path::new(project_dir).join("plutus.json"),
    };
    if !path.exists() {
        return Err(BlueprintError::BlueprintNotFound(
            path.display().to_string(),
        ));
    }
    let content = std::fs::read_to_string(&path)?;
    let blueprint: FullBlueprint = serde_json::from_str(&content)
        .map_err(|e| BlueprintError::BlueprintParse(e.to_string()))?;
    Ok(blueprint)
}

fn find_validator<'a>(
    blueprint: &'a FullBlueprint,
    validator: &str,
) -> Result<&'a FullValidator, BlueprintError> {
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
        .ok_or_else(|| BlueprintError::ValidatorNotFound(validator.to_string()))
}

/// Parse a validator title like "module.function.purpose" into (full_name, purpose).
fn parse_validator_title(title: &str) -> (String, String) {
    let parts: Vec<&str> = title.split('.').collect();
    if parts.len() >= 3 {
        // e.g. "escrow.escrow.spend" -> name="escrow.spend", purpose="spend"
        let purpose = parts.last().copied().unwrap_or("spend").to_string();
        (title.to_string(), purpose)
    } else if parts.len() == 2 {
        let purpose = infer_purpose(parts[1]);
        (title.to_string(), purpose)
    } else {
        (title.to_string(), "spend".to_string())
    }
}

fn infer_purpose(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("mint") || lower.contains("policy") {
        "mint".to_string()
    } else if lower.contains("withdraw") {
        "withdraw".to_string()
    } else if lower.contains("publish") || lower.contains("cert") {
        "publish".to_string()
    } else {
        "spend".to_string()
    }
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
    fn test_blueprint_show_escrow() -> TestResult {
        let dir = fixture_dir();
        let output = blueprint_show(dir_str(&dir), None)?;
        assert_eq!(output.data.preamble.title, "test/escrow");
        assert_eq!(output.data.preamble.plutus_version, "v3");
        assert_eq!(output.data.validators.len(), 2);

        let v0 = &output.data.validators[0];
        assert_eq!(v0.name, "escrow.escrow.spend");
        assert_eq!(v0.purpose, "spend");
        assert!(!v0.hash.is_empty());
        assert!(v0.datum_schema.is_some());
        assert!(v0.redeemer_schema.is_some());
        assert!(!v0.parameterized);

        let v1 = &output.data.validators[1];
        assert_eq!(v1.name, "escrow.token.mint");
        assert_eq!(v1.purpose, "mint");
        assert!(v1.datum_schema.is_none());
        Ok(())
    }

    #[test]
    fn test_blueprint_show_missing_file() {
        let result = blueprint_show("/nonexistent/path", None);
        assert!(matches!(result, Err(BlueprintError::BlueprintNotFound(_))));
    }

    #[test]
    fn test_blueprint_apply_non_parameterized() -> TestResult {
        let dir = fixture_dir();
        let result = blueprint_apply(
            dir_str(&dir),
            None,
            Some("escrow.escrow.spend"),
            Some(r#"{"param1": 42}"#),
        );
        assert!(matches!(result, Err(BlueprintError::NotParameterized)));
        Ok(())
    }

    #[test]
    fn test_blueprint_apply_missing_validator() {
        let result = blueprint_apply(".", None, None, Some(r#"{"p": 1}"#));
        assert!(matches!(result, Err(BlueprintError::ValidatorRequired)));
    }

    #[test]
    fn test_blueprint_apply_missing_params() {
        let dir = fixture_dir();
        let result = blueprint_apply(dir_str(&dir), None, Some("0"), None);
        assert!(matches!(result, Err(BlueprintError::ParamsRequired)));
    }

    #[test]
    fn test_blueprint_convert_produces_text_envelope() -> TestResult {
        let dir = fixture_dir();
        let output = blueprint_convert(dir_str(&dir), None, Some("0"), None)?;
        assert_eq!(output.data.text_envelope.type_field, "PlutusScriptV3");
        assert!(output.data.text_envelope.description.is_empty());
        assert_eq!(output.data.text_envelope.cbor_hex, "deadbeef");
        assert!(output.data.written_to.is_none());
        Ok(())
    }

    #[test]
    fn test_blueprint_convert_writes_file() -> TestResult {
        let dir = fixture_dir();
        let tmp = std::env::temp_dir().join("utxray-test-convert.json");
        let tmp_str = tmp.to_str().ok_or("invalid path")?;
        let output = blueprint_convert(dir_str(&dir), None, Some("0"), Some(tmp_str))?;
        assert_eq!(output.data.written_to.as_deref(), Some(tmp_str));

        let content = std::fs::read_to_string(&tmp)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        assert_eq!(parsed["type"], "PlutusScriptV3");
        assert_eq!(parsed["cborHex"], "deadbeef");

        let _ = std::fs::remove_file(&tmp);
        Ok(())
    }

    #[test]
    fn test_blueprint_convert_missing_validator() {
        let dir = fixture_dir();
        let result = blueprint_convert(dir_str(&dir), None, Some("nonexistent"), None);
        assert!(matches!(result, Err(BlueprintError::ValidatorNotFound(_))));
    }

    #[test]
    fn test_blueprint_convert_validator_required() {
        let dir = fixture_dir();
        let result = blueprint_convert(dir_str(&dir), None, None, None);
        assert!(matches!(result, Err(BlueprintError::ValidatorRequired)));
    }

    #[test]
    fn test_parse_validator_title_three_parts() {
        let (name, purpose) = parse_validator_title("escrow.escrow.spend");
        assert_eq!(name, "escrow.escrow.spend");
        assert_eq!(purpose, "spend");
    }

    #[test]
    fn test_parse_validator_title_two_parts_mint() {
        let (name, purpose) = parse_validator_title("escrow.token_mint");
        assert_eq!(name, "escrow.token_mint");
        assert_eq!(purpose, "mint");
    }
}
