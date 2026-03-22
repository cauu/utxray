use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum GenContextError {
    #[error("blueprint not found: {0}")]
    BlueprintNotFound(String),

    #[error("blueprint parse error: {0}")]
    BlueprintParse(String),

    #[error("failed to read aiken.toml: {0}")]
    AikenTomlReadError(String),

    #[error("failed to write context file: {0}")]
    WriteError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextFile {
    pub validators: Vec<ContextValidator>,
    pub project: ContextProject,
    pub generated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextValidator {
    pub name: String,
    pub purpose: String,
    pub hash: String,
    pub test_status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextProject {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct GenContextOutput {
    pub path: String,
    pub validators_count: usize,
    pub project: ContextProject,
}

// ── Public API ─────────────────────────────────────────────────

/// Generate the `.utxray/context.json` file.
///
/// Reads plutus.json for validator info and aiken.toml for project info.
/// `output_path` overrides the default `.utxray/context.json`.
pub fn gen_context(
    project_dir: &str,
    output_path: Option<&str>,
) -> Result<Output<GenContextOutput>, GenContextError> {
    let blueprint_path = Path::new(project_dir).join("plutus.json");
    if !blueprint_path.exists() {
        return Err(GenContextError::BlueprintNotFound(
            blueprint_path.display().to_string(),
        ));
    }

    let bp_content = std::fs::read_to_string(&blueprint_path)?;
    let bp: serde_json::Value = serde_json::from_str(&bp_content)
        .map_err(|e| GenContextError::BlueprintParse(e.to_string()))?;

    // Extract validators
    let validators = extract_validators(&bp);

    // Extract project info from aiken.toml (fallback to blueprint preamble)
    let project = read_project_info(project_dir, &bp);

    let generated_at = chrono::Utc::now().to_rfc3339();

    let context = ContextFile {
        validators: validators.clone(),
        project: project.clone(),
        generated_at,
    };

    // Determine output path
    let default_path = Path::new(project_dir).join(".utxray").join("context.json");
    let dest = match output_path {
        Some(p) => Path::new(p).to_path_buf(),
        None => default_path,
    };

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&context)
        .map_err(|e| GenContextError::WriteError(e.to_string()))?;
    std::fs::write(&dest, &json)?;

    Ok(Output::ok(GenContextOutput {
        path: dest.display().to_string(),
        validators_count: context.validators.len(),
        project,
    }))
}

// ── Internal helpers ───────────────────────────────────────────

fn extract_validators(bp: &serde_json::Value) -> Vec<ContextValidator> {
    let empty_vec = vec![];
    let validator_array = bp
        .get("validators")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    validator_array
        .iter()
        .map(|v| {
            let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("unknown");
            let hash = v.get("hash").and_then(|h| h.as_str()).unwrap_or("");
            let (name, purpose) = parse_validator_title(title);

            ContextValidator {
                name,
                purpose,
                hash: hash.to_string(),
                test_status: "unknown".to_string(),
            }
        })
        .collect()
}

fn read_project_info(project_dir: &str, bp: &serde_json::Value) -> ContextProject {
    // Try aiken.toml first
    let aiken_path = Path::new(project_dir).join("aiken.toml");
    if let Ok(content) = std::fs::read_to_string(&aiken_path) {
        if let Ok(toml_val) = content.parse::<toml::Table>() {
            let name = toml_val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let version = toml_val
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string();
            return ContextProject { name, version };
        }
    }

    // Fallback to blueprint preamble
    let preamble = bp.get("preamble");
    let name = preamble
        .and_then(|p| p.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version = preamble
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    ContextProject { name, version }
}

fn parse_validator_title(title: &str) -> (String, String) {
    let parts: Vec<&str> = title.split('.').collect();
    if parts.len() >= 3 {
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
    fn test_gen_context_success() -> TestResult {
        let dir = fixture_dir();
        let tmp_dir = std::env::temp_dir().join("utxray-test-gen-context");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir)?;
        let out_path = tmp_dir.join("context.json");
        let out_str = out_path.to_str().ok_or("invalid path")?;

        let output = gen_context(dir_str(&dir), Some(out_str))?;
        assert_eq!(output.data.validators_count, 2);
        assert!(out_path.exists());

        // Verify written file is valid JSON
        let content = std::fs::read_to_string(&out_path)?;
        let parsed: ContextFile = serde_json::from_str(&content)?;
        assert_eq!(parsed.validators.len(), 2);
        assert_eq!(parsed.validators[0].name, "escrow.escrow.spend");
        assert_eq!(parsed.validators[0].purpose, "spend");
        assert_eq!(parsed.validators[0].test_status, "unknown");
        assert_eq!(parsed.validators[1].name, "escrow.token.mint");
        assert_eq!(parsed.validators[1].purpose, "mint");

        let _ = std::fs::remove_dir_all(&tmp_dir);
        Ok(())
    }

    #[test]
    fn test_gen_context_idempotent() -> TestResult {
        let dir = fixture_dir();
        let tmp_dir = std::env::temp_dir().join("utxray-test-gen-context-idem");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir)?;
        let out_path = tmp_dir.join("context.json");
        let out_str = out_path.to_str().ok_or("invalid path")?;

        // Run twice
        let _output1 = gen_context(dir_str(&dir), Some(out_str))?;
        let output2 = gen_context(dir_str(&dir), Some(out_str))?;

        assert_eq!(output2.data.validators_count, 2);
        // File should still be valid JSON
        let content = std::fs::read_to_string(&out_path)?;
        let parsed: ContextFile = serde_json::from_str(&content)?;
        assert_eq!(parsed.validators.len(), 2);

        let _ = std::fs::remove_dir_all(&tmp_dir);
        Ok(())
    }

    #[test]
    fn test_gen_context_missing_blueprint() {
        let result = gen_context("/nonexistent/path", None);
        assert!(matches!(result, Err(GenContextError::BlueprintNotFound(_))));
    }

    #[test]
    fn test_extract_validators() {
        let bp = serde_json::json!({
            "validators": [
                {"title": "mod.val.spend", "hash": "abc123"},
                {"title": "mod.token.mint", "hash": "def456"}
            ]
        });
        let validators = extract_validators(&bp);
        assert_eq!(validators.len(), 2);
        assert_eq!(validators[0].name, "mod.val.spend");
        assert_eq!(validators[0].purpose, "spend");
        assert_eq!(validators[1].purpose, "mint");
    }

    #[test]
    fn test_read_project_info_from_blueprint() {
        let bp = serde_json::json!({
            "preamble": {
                "title": "my-project",
                "version": "1.0.0"
            }
        });
        let project = read_project_info("/nonexistent", &bp);
        assert_eq!(project.name, "my-project");
        assert_eq!(project.version, "1.0.0");
    }
}
