use serde::Serialize;

use crate::error::{Confidence, ErrorCode, Severity};

/// A classification rule that maps patterns to error codes.
struct Rule {
    keywords: &'static [&'static str],
    error_code: ErrorCode,
    severity: Severity,
    category: &'static str,
    confidence: Confidence,
    suggested_commands: &'static [&'static str],
}

/// All built-in classification rules, ordered by specificity (most specific first).
const RULES: &[Rule] = &[
    Rule {
        keywords: &["redeemer", "index", "mismatch"],
        error_code: ErrorCode::RedeemerIndexMismatch,
        severity: Severity::Critical,
        category: "cbor_schema",
        confidence: Confidence::High,
        suggested_commands: &[
            "utxray redeemer-index --tx ./tx.cbor",
            "utxray cbor decode --hex <redeemer_cbor>",
        ],
    },
    Rule {
        keywords: &["script", "data", "hash"],
        error_code: ErrorCode::ScriptDataHashMismatch,
        severity: Severity::Critical,
        category: "cbor_schema",
        confidence: Confidence::High,
        suggested_commands: &["utxray script-data-hash --tx ./tx.cbor"],
    },
    Rule {
        keywords: &["budget", "exceeded"],
        error_code: ErrorCode::Phase2BudgetExceeded,
        severity: Severity::Critical,
        category: "execution",
        confidence: Confidence::High,
        suggested_commands: &["utxray budget summary --from <result.json>"],
    },
    Rule {
        keywords: &["collateral", "missing"],
        error_code: ErrorCode::Phase1CollateralMissing,
        severity: Severity::Critical,
        category: "phase1",
        confidence: Confidence::High,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["required", "signer", "missing"],
        error_code: ErrorCode::Phase1RequiredSignerMissing,
        severity: Severity::Critical,
        category: "phase1",
        confidence: Confidence::High,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["datum", "not", "found"],
        error_code: ErrorCode::DatumNotFound,
        severity: Severity::Critical,
        category: "datum",
        confidence: Confidence::High,
        suggested_commands: &[
            "utxray datum decode --utxo <txhash#idx>",
            "utxray schema validate --datum <datum.json> --validator <name>",
        ],
    },
    Rule {
        keywords: &["deadline"],
        error_code: ErrorCode::ValidityIntervalFail,
        severity: Severity::Warning,
        category: "validity",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray context tip"],
    },
    Rule {
        keywords: &["validity", "interval"],
        error_code: ErrorCode::ValidityIntervalFail,
        severity: Severity::Warning,
        category: "validity",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray context tip"],
    },
    Rule {
        keywords: &["signature"],
        error_code: ErrorCode::Phase1RequiredSignerMissing,
        severity: Severity::Critical,
        category: "phase1",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["schema", "mismatch"],
        error_code: ErrorCode::SchemaMismatch,
        severity: Severity::Critical,
        category: "cbor_schema",
        confidence: Confidence::High,
        suggested_commands: &["utxray schema validate --datum <datum.json> --validator <name>"],
    },
    Rule {
        keywords: &["type", "mismatch"],
        error_code: ErrorCode::TypeMismatch,
        severity: Severity::Critical,
        category: "type_error",
        confidence: Confidence::High,
        suggested_commands: &["utxray typecheck"],
    },
    Rule {
        keywords: &["constructor", "index"],
        error_code: ErrorCode::ConstructorIndexWrong,
        severity: Severity::Critical,
        category: "cbor_schema",
        confidence: Confidence::High,
        suggested_commands: &[
            "utxray schema validate --datum <datum.json> --validator <name>",
            "utxray cbor decode --hex <datum_cbor>",
        ],
    },
    Rule {
        keywords: &["balance", "error"],
        error_code: ErrorCode::Phase1BalanceError,
        severity: Severity::Critical,
        category: "phase1",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["min", "utxo"],
        error_code: ErrorCode::Phase1MinUtxoFail,
        severity: Severity::Warning,
        category: "phase1",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["tx", "size", "exceeded"],
        error_code: ErrorCode::Phase1TxSizeExceeded,
        severity: Severity::Warning,
        category: "phase1",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray tx build --from ./tx-spec.json"],
    },
    Rule {
        keywords: &["script", "fail"],
        error_code: ErrorCode::Phase2ScriptFail,
        severity: Severity::Critical,
        category: "execution",
        confidence: Confidence::Medium,
        suggested_commands: &[
            "utxray trace --validator <name> --purpose spend --redeemer <json>",
            "utxray diagnose --from <result.json>",
        ],
    },
    Rule {
        keywords: &["mint", "policy"],
        error_code: ErrorCode::MintPolicyFail,
        severity: Severity::Critical,
        category: "execution",
        confidence: Confidence::Medium,
        suggested_commands: &["utxray trace --validator <name> --purpose mint --redeemer <json>"],
    },
    Rule {
        keywords: &["already", "spent"],
        error_code: ErrorCode::SubmitAlreadySpent,
        severity: Severity::Critical,
        category: "submit",
        confidence: Confidence::High,
        suggested_commands: &["utxray utxo status --utxo <txhash#idx>"],
    },
];

/// Result of classifying an error from input JSON.
#[derive(Debug, Serialize)]
pub struct Classification {
    pub error_code: ErrorCode,
    pub severity: Severity,
    pub category: String,
    pub confidence: Confidence,
    pub source_command: String,
    pub matched_rules: Vec<String>,
    pub summary: String,
    pub evidence: serde_json::Value,
    pub suggested_commands: Vec<String>,
    pub related_errors: Vec<String>,
}

/// Classify an error from input JSON.
///
/// Examines traces, error messages, error_code, and result fields
/// to determine the most likely error classification.
pub fn classify(input: &serde_json::Value) -> Classification {
    let text = collect_searchable_text(input);
    let text_lower = text.to_lowercase();

    // Try to detect the source command
    let source_command = detect_source_command(input);

    // Check if there's already an error_code in the input
    if let Some(existing_code) = input.get("error_code").and_then(|v| v.as_str()) {
        let existing_lower = existing_code.to_lowercase().replace('_', " ");
        // Try to match existing code against our rules
        for rule in RULES {
            let rule_code_str = serde_json::to_string(&rule.error_code)
                .unwrap_or_default()
                .trim_matches('"')
                .to_lowercase()
                .replace('_', " ");
            if existing_lower.contains(&rule_code_str) || rule_code_str.contains(&existing_lower) {
                return Classification {
                    error_code: rule.error_code.clone(),
                    severity: rule.severity.clone(),
                    category: rule.category.to_string(),
                    confidence: Confidence::High,
                    source_command: source_command.clone(),
                    matched_rules: vec![format!("existing error_code: {existing_code}")],
                    summary: build_summary(&rule.error_code, input),
                    evidence: extract_evidence(input),
                    suggested_commands: rule
                        .suggested_commands
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                    related_errors: vec![],
                };
            }
        }
    }

    // Pattern match against rules
    let mut best_match: Option<(&Rule, Vec<String>)> = None;
    let mut best_keyword_count = 0;

    for rule in RULES {
        let matched_keywords: Vec<String> = rule
            .keywords
            .iter()
            .filter(|kw| text_lower.contains(&kw.to_lowercase()))
            .map(|kw| (*kw).to_string())
            .collect();

        if matched_keywords.len() == rule.keywords.len()
            && matched_keywords.len() > best_keyword_count
        {
            best_keyword_count = matched_keywords.len();
            best_match = Some((rule, matched_keywords));
        }
    }

    if let Some((rule, matched_kws)) = best_match {
        let matched_rules: Vec<String> = matched_kws
            .iter()
            .map(|kw| format!("matched keyword: {kw}"))
            .collect();

        return Classification {
            error_code: rule.error_code.clone(),
            severity: rule.severity.clone(),
            category: rule.category.to_string(),
            confidence: rule.confidence.clone(),
            source_command,
            matched_rules,
            summary: build_summary(&rule.error_code, input),
            evidence: extract_evidence(input),
            suggested_commands: rule
                .suggested_commands
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            related_errors: vec![],
        };
    }

    // No match — return unknown
    Classification {
        error_code: ErrorCode::UnknownError,
        severity: Severity::Warning,
        category: "unknown".to_string(),
        confidence: Confidence::Low,
        source_command,
        matched_rules: vec![],
        summary: "Could not classify the error. Manual inspection recommended.".to_string(),
        evidence: extract_evidence(input),
        suggested_commands: vec!["utxray diagnose --from <result.json>".to_string()],
        related_errors: vec![],
    }
}

/// Collect all text fields from the JSON that we should search through.
fn collect_searchable_text(input: &serde_json::Value) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Top-level message/error fields
    if let Some(msg) = input.get("message").and_then(|v| v.as_str()) {
        parts.push(msg.to_string());
    }
    if let Some(code) = input.get("error_code").and_then(|v| v.as_str()) {
        parts.push(code.replace('_', " "));
    }
    if let Some(summary) = input.get("summary").and_then(|v| v.as_str()) {
        parts.push(summary.to_string());
    }

    // Traces array
    if let Some(traces) = input.get("traces").and_then(|v| v.as_array()) {
        for t in traces {
            if let Some(s) = t.as_str() {
                parts.push(s.to_string());
            }
        }
    }

    // Errors array
    if let Some(errors) = input.get("errors").and_then(|v| v.as_array()) {
        for e in errors {
            if let Some(s) = e.as_str() {
                parts.push(s.to_string());
            }
            if let Some(msg) = e.get("message").and_then(|v| v.as_str()) {
                parts.push(msg.to_string());
            }
        }
    }

    // Test results
    if let Some(results) = input.get("results").and_then(|v| v.as_array()) {
        for r in results {
            if let Some(traces) = r.get("traces").and_then(|v| v.as_array()) {
                for t in traces {
                    if let Some(s) = t.as_str() {
                        parts.push(s.to_string());
                    }
                }
            }
            if let Some(err) = r.get("error_detail").and_then(|v| v.as_str()) {
                parts.push(err.to_string());
            }
        }
    }

    // error_detail at top level
    if let Some(detail) = input.get("error_detail").and_then(|v| v.as_str()) {
        parts.push(detail.to_string());
    }

    // Nested execution.result
    if let Some(execution) = input.get("execution") {
        if let Some(result) = execution.get("result") {
            parts.push(result.to_string());
        }
    }

    parts.join(" ")
}

/// Detect which command produced this result.
fn detect_source_command(input: &serde_json::Value) -> String {
    // Check execution.command field (from bundles)
    if let Some(exec) = input.get("execution") {
        if let Some(cmd) = exec.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
    }

    // Infer from structure
    if input.get("results").is_some() && input.get("total").is_some() {
        return "test".to_string();
    }
    if input.get("traces").is_some() && input.get("validator").is_some() {
        return "trace".to_string();
    }
    if input.get("tx_hash").is_some() {
        return "tx.simulate".to_string();
    }

    "unknown".to_string()
}

/// Build a human-readable summary from the error code and input.
fn build_summary(error_code: &ErrorCode, input: &serde_json::Value) -> String {
    let base = error_code.to_string();

    // Try to add context from the input
    if let Some(msg) = input.get("message").and_then(|v| v.as_str()) {
        return format!("{base}: {msg}");
    }
    if let Some(detail) = input.get("error_detail").and_then(|v| v.as_str()) {
        return format!("{base}: {detail}");
    }

    base
}

/// Extract relevant evidence fields from the input.
fn extract_evidence(input: &serde_json::Value) -> serde_json::Value {
    let mut evidence = serde_json::Map::new();

    if let Some(traces) = input.get("traces") {
        evidence.insert("traces".to_string(), traces.clone());
    }
    if let Some(errors) = input.get("errors") {
        evidence.insert("errors".to_string(), errors.clone());
    }
    if let Some(msg) = input.get("message") {
        evidence.insert("message".to_string(), msg.clone());
    }
    if let Some(detail) = input.get("error_detail") {
        evidence.insert("error_detail".to_string(), detail.clone());
    }
    if let Some(results) = input.get("results") {
        evidence.insert("results".to_string(), results.clone());
    }

    serde_json::Value::Object(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_redeemer_index_mismatch() {
        let input = serde_json::json!({
            "message": "Redeemer at index 0 mismatch with input",
            "traces": ["redeemer index does not match expected input"]
        });
        let result = classify(&input);
        assert!(matches!(
            result.error_code,
            ErrorCode::RedeemerIndexMismatch
        ));
        assert!(matches!(result.severity, Severity::Critical));
        assert_eq!(result.category, "cbor_schema");
    }

    #[test]
    fn test_classify_budget_exceeded() {
        let input = serde_json::json!({
            "error_detail": "Execution budget exceeded for script",
            "traces": ["budget exceeded at step 1000"]
        });
        let result = classify(&input);
        assert!(matches!(result.error_code, ErrorCode::Phase2BudgetExceeded));
    }

    #[test]
    fn test_classify_deadline_validity() {
        let input = serde_json::json!({
            "traces": ["deadline check failed: current slot past deadline"]
        });
        let result = classify(&input);
        assert!(matches!(result.error_code, ErrorCode::ValidityIntervalFail));
        assert!(matches!(result.confidence, Confidence::Medium));
    }

    #[test]
    fn test_classify_existing_error_code() {
        let input = serde_json::json!({
            "error_code": "REDEEMER_INDEX_MISMATCH",
            "message": "some details"
        });
        let result = classify(&input);
        assert!(matches!(
            result.error_code,
            ErrorCode::RedeemerIndexMismatch
        ));
        assert!(matches!(result.confidence, Confidence::High));
    }

    #[test]
    fn test_classify_unknown() {
        let input = serde_json::json!({
            "message": "something completely unrecognized happened"
        });
        let result = classify(&input);
        assert!(matches!(result.error_code, ErrorCode::UnknownError));
        assert!(matches!(result.confidence, Confidence::Low));
    }

    #[test]
    fn test_classify_from_test_results() {
        let input = serde_json::json!({
            "total": 5,
            "results": [
                {
                    "result": "fail",
                    "traces": ["datum not found in reference inputs"],
                    "error_detail": "datum not found"
                }
            ]
        });
        let result = classify(&input);
        assert!(matches!(result.error_code, ErrorCode::DatumNotFound));
        assert_eq!(result.source_command, "test");
    }

    #[test]
    fn test_classify_schema_mismatch() {
        let input = serde_json::json!({
            "message": "schema mismatch between datum and blueprint"
        });
        let result = classify(&input);
        assert!(matches!(result.error_code, ErrorCode::SchemaMismatch));
    }

    #[test]
    fn test_detect_source_command_test() {
        let input = serde_json::json!({ "results": [], "total": 0 });
        assert_eq!(detect_source_command(&input), "test");
    }

    #[test]
    fn test_detect_source_command_trace() {
        let input = serde_json::json!({ "traces": [], "validator": "foo" });
        assert_eq!(detect_source_command(&input), "trace");
    }

    #[test]
    fn test_detect_source_command_from_execution() {
        let input = serde_json::json!({ "execution": { "command": "tx.simulate" } });
        assert_eq!(detect_source_command(&input), "tx.simulate");
    }

    #[test]
    fn test_collect_searchable_text_nested_errors() {
        let input = serde_json::json!({
            "errors": [
                { "message": "collateral missing from transaction" }
            ]
        });
        let text = collect_searchable_text(&input);
        assert!(text.contains("collateral missing"));
    }

    #[test]
    fn test_evidence_extraction() {
        let input = serde_json::json!({
            "traces": ["a", "b"],
            "message": "test",
            "unrelated_field": 42
        });
        let evidence = extract_evidence(&input);
        assert!(evidence.get("traces").is_some());
        assert!(evidence.get("message").is_some());
        assert!(evidence.get("unrelated_field").is_none());
    }
}
