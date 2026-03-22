use serde::Serialize;

use crate::error::Severity;

pub const UTXRAY_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Error,
    Mixed,
}

/// Validation/execution result (named Outcome to avoid collision with std::Result)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
}

/// All command output goes through this wrapper, ensuring consistent top-level fields
#[derive(Debug, Serialize)]
pub struct Output<T: Serialize> {
    pub v: String,
    pub status: Status,
    #[serde(flatten)]
    pub data: T,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Serialize)]
pub struct Warning {
    pub severity: Severity,
    pub message: String,
}

impl<T: Serialize> Output<T> {
    pub fn ok(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Ok,
            data,
            warnings: vec![],
        }
    }

    pub fn mixed(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Mixed,
            data,
            warnings: vec![],
        }
    }

    pub fn error(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Error,
            data,
            warnings: vec![],
        }
    }

    pub fn with_warning(mut self, severity: Severity, msg: impl Into<String>) -> Self {
        self.warnings.push(Warning {
            severity,
            message: msg.into(),
        });
        self
    }
}

/// Output to stdout in JSON format (default).
/// Prefer `print_output_formatted` to respect the user's `--format` flag.
pub fn print_output<T: Serialize>(output: &Output<T>) -> std::result::Result<(), anyhow::Error> {
    print_output_formatted(output, "json")
}

/// Output to stdout with the specified format.
/// `format` is "json" (default) or "text" (simplified key: value rendering).
pub fn print_output_formatted<T: Serialize>(
    output: &Output<T>,
    format: &str,
) -> std::result::Result<(), anyhow::Error> {
    match format {
        "text" => {
            // Serialize to JSON Value first, then render as text
            let value = serde_json::to_value(output)?;
            println!(
                "status: {}",
                value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            );
            println!(
                "v: {}",
                value.get("v").and_then(|v| v.as_str()).unwrap_or("unknown")
            );
            if let Some(obj) = value.as_object() {
                for (key, val) in obj {
                    if key == "status" || key == "v" || key == "warnings" {
                        continue;
                    }
                    match val {
                        serde_json::Value::String(s) => println!("{key}: {s}"),
                        serde_json::Value::Number(n) => println!("{key}: {n}"),
                        serde_json::Value::Bool(b) => println!("{key}: {b}"),
                        serde_json::Value::Null => println!("{key}: null"),
                        _ => {
                            // For complex values, use compact JSON
                            let compact = serde_json::to_string(val)?;
                            println!("{key}: {compact}");
                        }
                    }
                }
            }
            if !output.warnings.is_empty() {
                for w in &output.warnings {
                    println!("warning: {}", w.message);
                }
            }
            Ok(())
        }
        _ => {
            // Default: JSON
            let json = serde_json::to_string_pretty(output)?;
            println!("{json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_output_ok_serialization() -> TestResult {
        let output = Output::ok(serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&output)?;
        let parsed: serde_json::Value = serde_json::from_str(&json)?;
        assert_eq!(parsed["v"], "0.1.0");
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["key"], "value");
        assert!(parsed.get("warnings").is_none());
        Ok(())
    }

    #[test]
    fn test_output_error_serialization() -> TestResult {
        let output = Output::error(serde_json::json!({"error_code": "TEST"}));
        let json = serde_json::to_string(&output)?;
        let parsed: serde_json::Value = serde_json::from_str(&json)?;
        assert_eq!(parsed["status"], "error");
        assert_eq!(parsed["error_code"], "TEST");
        Ok(())
    }

    #[test]
    fn test_output_with_warnings() -> TestResult {
        let output =
            Output::ok(serde_json::json!({})).with_warning(Severity::Warning, "test warning");
        let json = serde_json::to_string(&output)?;
        let parsed: serde_json::Value = serde_json::from_str(&json)?;
        assert!(parsed["warnings"].is_array());
        assert_eq!(parsed["warnings"][0]["severity"], "warning");
        assert_eq!(parsed["warnings"][0]["message"], "test warning");
        Ok(())
    }

    #[test]
    fn test_outcome_serialization() -> TestResult {
        let pass = serde_json::to_string(&Outcome::Pass)?;
        let fail = serde_json::to_string(&Outcome::Fail)?;
        assert_eq!(pass, "\"pass\"");
        assert_eq!(fail, "\"fail\"");
        Ok(())
    }

    #[test]
    fn test_status_serialization() -> TestResult {
        assert_eq!(serde_json::to_string(&Status::Ok)?, "\"ok\"");
        assert_eq!(serde_json::to_string(&Status::Error)?, "\"error\"");
        assert_eq!(serde_json::to_string(&Status::Mixed)?, "\"mixed\"");
        Ok(())
    }
}
