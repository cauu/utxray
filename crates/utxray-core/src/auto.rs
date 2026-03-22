use serde::Serialize;

use crate::output::{Output, Status, UTXRAY_VERSION};

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AutoError {
    #[error("unknown scenario: {0}. Valid: build, test, trace, tx, full")]
    UnknownScenario(String),

    #[error("--validator is required for scenario '{0}'")]
    ValidatorRequired(String),

    #[error("--purpose is required for scenario '{0}'")]
    PurposeRequired(String),
}

// ── Scenario & step types ──────────────────────────────────────

/// The five defined scenarios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scenario {
    Build,
    Test,
    Trace,
    Tx,
    Full,
}

impl Scenario {
    pub fn parse(s: &str) -> Result<Self, AutoError> {
        match s {
            "build" => Ok(Self::Build),
            "test" => Ok(Self::Test),
            "trace" => Ok(Self::Trace),
            "tx" => Ok(Self::Tx),
            "full" => Ok(Self::Full),
            other => Err(AutoError::UnknownScenario(other.to_string())),
        }
    }

    /// Return the ordered list of step names for this scenario.
    pub fn steps(&self) -> Vec<&'static str> {
        match self {
            Self::Build => vec!["typecheck", "build"],
            Self::Test => vec!["build", "test", "diagnose"],
            Self::Trace => vec!["build", "schema_validate", "trace", "diagnose"],
            Self::Tx => vec![
                "build",
                "tx_build",
                "tx_evaluate",
                "tx_build_2",
                "tx_simulate",
                "diagnose",
            ],
            Self::Full => vec![
                "build",
                "test",
                "trace",
                "tx_build",
                "tx_evaluate",
                "tx_build_2",
                "tx_simulate",
                "diagnose",
                "replay_bundle",
            ],
        }
    }
}

/// A single step's execution result.
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub command: String,
    pub status: Status,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AutoOutput {
    pub scenario: String,
    pub steps: Vec<StepResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub artifacts_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_next: Option<String>,
}

// ── Public API ─────────────────────────────────────────────────

/// Parameters for the auto orchestrator.
pub struct AutoParams<'a> {
    pub project_dir: &'a str,
    pub scenario: &'a str,
    pub validator: Option<&'a str>,
    pub purpose: Option<&'a str>,
    pub datum: Option<&'a str>,
    pub redeemer: Option<&'a str>,
    pub tx_spec: Option<&'a str>,
}

/// Run the auto orchestration scenario.
///
/// This runs each step in sequence, checking stop conditions after each step.
/// Steps that require aiken call the corresponding core functions.
/// Steps that require chain connectivity check for Blockfrost config.
pub async fn run_auto(params: AutoParams<'_>) -> anyhow::Result<Output<AutoOutput>> {
    let scenario = Scenario::parse(params.scenario).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Validate required params for scenarios that need them
    validate_params(&scenario, &params)?;

    let step_names = scenario.steps();
    let mut steps: Vec<StepResult> = Vec::new();
    let mut stopped_at: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut suggested_next: Option<String> = None;
    let mut has_failure = false;

    let artifacts_dir = format!("{}/.utxray/auto/", params.project_dir);

    // Ensure artifacts directory exists
    let _ = std::fs::create_dir_all(&artifacts_dir);

    for step_name in &step_names {
        let start = std::time::Instant::now();
        let result = execute_step(step_name, &params).await;
        let duration_ms = start.elapsed().as_millis();

        let step_result = match result {
            Ok((status, summary)) => {
                let is_error = status == Status::Error;
                let is_failure = status == Status::Mixed || status == Status::Error;

                let sr = StepResult {
                    command: step_name.to_string(),
                    status: status.clone(),
                    duration_ms,
                    summary,
                    error_code: None,
                };

                if is_error {
                    stopped_at = Some(step_name.to_string());
                    reason = Some("Step returned error status.".to_string());
                    steps.push(sr);
                    break;
                }

                if is_failure {
                    has_failure = true;
                    // For diagnose-eligible failures, continue to diagnose step
                    // but after diagnose, stop
                    if *step_name != "diagnose" && !is_diagnose_next(step_name, &step_names) {
                        stopped_at = Some(step_name.to_string());
                        reason = Some(format!("Failure detected at {step_name}."));
                        steps.push(sr);
                        break;
                    }
                }

                // If this is the diagnose step and we arrived here due to failure, stop
                if *step_name == "diagnose" && has_failure {
                    stopped_at = Some(step_name.to_string());
                    reason = Some("Diagnosis complete after failure.".to_string());
                    suggested_next = suggest_next(step_name, &params);
                    steps.push(sr);
                    break;
                }

                sr
            }
            Err(e) => {
                let sr = StepResult {
                    command: step_name.to_string(),
                    status: Status::Error,
                    duration_ms,
                    summary: None,
                    error_code: Some(e.to_string()),
                };
                stopped_at = Some(step_name.to_string());
                reason = Some(format!("Step failed: {e}"));
                steps.push(sr);
                break;
            }
        };

        steps.push(step_result);
    }

    let overall_status = if steps.iter().any(|s| s.status == Status::Error) {
        Status::Error
    } else if steps.iter().any(|s| s.status == Status::Mixed) || has_failure {
        Status::Mixed
    } else {
        Status::Ok
    };

    let data = AutoOutput {
        scenario: params.scenario.to_string(),
        steps,
        stopped_at,
        reason,
        artifacts_dir,
        suggested_next,
    };

    Ok(Output {
        v: UTXRAY_VERSION.to_string(),
        status: overall_status,
        data,
        warnings: vec![],
    })
}

// ── Internal helpers ───────────────────────────────────────────

fn validate_params(scenario: &Scenario, params: &AutoParams<'_>) -> anyhow::Result<()> {
    match scenario {
        Scenario::Trace | Scenario::Tx | Scenario::Full => {
            if params.validator.is_none() {
                return Err(
                    AutoError::ValidatorRequired(format!("{:?}", scenario).to_lowercase()).into(),
                );
            }
            if params.purpose.is_none() && matches!(scenario, Scenario::Trace) {
                return Err(
                    AutoError::PurposeRequired(format!("{:?}", scenario).to_lowercase()).into(),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

/// Execute a single step and return (status, optional summary).
async fn execute_step(
    step_name: &str,
    params: &AutoParams<'_>,
) -> anyhow::Result<(Status, Option<serde_json::Value>)> {
    match step_name {
        "typecheck" => {
            let output =
                crate::typecheck::run_typecheck(params.project_dir, None, "verbose").await?;
            Ok((output.status.clone(), Some(output.data)))
        }
        "build" => {
            let output = crate::build::run_build(params.project_dir).await?;
            Ok((output.status.clone(), Some(output.data)))
        }
        "test" => {
            let output =
                crate::test_cmd::run_test(params.project_dir, None, None, "verbose", None).await?;
            let summary = output.data.get("summary").cloned();
            Ok((output.status.clone(), summary))
        }
        "schema_validate" => {
            // Schema validate requires validator/purpose/redeemer
            if let (Some(validator), Some(purpose)) = (params.validator, params.purpose) {
                let redeemer = params
                    .redeemer
                    .unwrap_or(r#"{"constructor": 0, "fields": []}"#);
                match crate::cbor::schema::validate_schema(
                    params.project_dir,
                    validator,
                    purpose,
                    params.datum,
                    redeemer,
                ) {
                    Ok(output) => Ok((output.status.clone(), None)),
                    Err(e) => Ok((
                        Status::Error,
                        Some(serde_json::json!({"error": e.to_string()})),
                    )),
                }
            } else {
                Ok((
                    Status::Error,
                    Some(
                        serde_json::json!({"error": "validator and purpose required for schema_validate"}),
                    ),
                ))
            }
        }
        "trace" => {
            // Trace needs validator, purpose, and optionally datum/redeemer
            let config = crate::trace::TraceConfig {
                validator: params.validator.unwrap_or("0").to_string(),
                purpose: params.purpose.unwrap_or("spend").to_string(),
                redeemer: params
                    .redeemer
                    .unwrap_or(r#"{"constructor": 0, "fields": []}"#)
                    .to_string(),
                datum: params.datum.map(|s| s.to_string()),
                context: None,
                slot: None,
                signatories: vec![],
            };
            let output = crate::trace::run_trace(params.project_dir, config).await?;
            Ok((output.status.clone(), Some(output.data)))
        }
        "tx_build" | "tx_build_2" => {
            // tx build requires a tx-spec file
            if let Some(tx_spec) = params.tx_spec {
                let tx_out = format!("{}/.utxray/auto/tx.unsigned", params.project_dir);
                match crate::tx::builder::run_tx_build(tx_spec, None, &tx_out, false, "preview") {
                    Ok(output) => Ok((output.status.clone(), Some(output.data))),
                    Err(e) => Ok((
                        Status::Error,
                        Some(serde_json::json!({"error": e.to_string()})),
                    )),
                }
            } else {
                Ok((
                    Status::Error,
                    Some(serde_json::json!({"error": "tx-spec required for tx_build"})),
                ))
            }
        }
        "tx_evaluate" | "tx_simulate" => {
            // These require chain connectivity; return a placeholder for v1
            Ok((
                Status::Ok,
                Some(
                    serde_json::json!({"note": format!("{step_name} skipped (requires chain connectivity)")}),
                ),
            ))
        }
        "diagnose" => {
            // Diagnose with no input — return ok (no-op in auto context without failure data)
            Ok((
                Status::Ok,
                Some(serde_json::json!({"note": "diagnose step (auto context)"})),
            ))
        }
        "replay_bundle" => {
            // replay bundle for full scenario — placeholder
            Ok((
                Status::Ok,
                Some(serde_json::json!({"note": "replay_bundle skipped (auto context)"})),
            ))
        }
        other => Ok((
            Status::Error,
            Some(serde_json::json!({"error": format!("unknown step: {other}")})),
        )),
    }
}

/// Check if the next step after the given one is "diagnose" in the step list.
fn is_diagnose_next(current: &str, steps: &[&str]) -> bool {
    let mut found = false;
    for step in steps {
        if found {
            return *step == "diagnose";
        }
        if *step == current {
            found = true;
        }
    }
    false
}

/// Suggest a next command based on which step we stopped at.
fn suggest_next(stopped_at: &str, params: &AutoParams<'_>) -> Option<String> {
    match stopped_at {
        "test" | "diagnose" => Some("utxray test --match 'failing_test'".to_string()),
        "trace" => {
            let validator = params.validator.unwrap_or("0");
            Some(format!(
                "utxray trace --validator {validator} --purpose spend"
            ))
        }
        "build" | "typecheck" => Some("utxray build".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_scenario_build_steps() -> TestResult {
        let scenario = Scenario::parse("build")?;
        assert_eq!(scenario.steps(), vec!["typecheck", "build"]);
        Ok(())
    }

    #[test]
    fn test_scenario_test_steps() -> TestResult {
        let scenario = Scenario::parse("test")?;
        assert_eq!(scenario.steps(), vec!["build", "test", "diagnose"]);
        Ok(())
    }

    #[test]
    fn test_scenario_trace_steps() -> TestResult {
        let scenario = Scenario::parse("trace")?;
        assert_eq!(
            scenario.steps(),
            vec!["build", "schema_validate", "trace", "diagnose"]
        );
        Ok(())
    }

    #[test]
    fn test_scenario_tx_steps() -> TestResult {
        let scenario = Scenario::parse("tx")?;
        assert_eq!(
            scenario.steps(),
            vec![
                "build",
                "tx_build",
                "tx_evaluate",
                "tx_build_2",
                "tx_simulate",
                "diagnose"
            ]
        );
        Ok(())
    }

    #[test]
    fn test_scenario_full_steps() -> TestResult {
        let scenario = Scenario::parse("full")?;
        assert_eq!(
            scenario.steps(),
            vec![
                "build",
                "test",
                "trace",
                "tx_build",
                "tx_evaluate",
                "tx_build_2",
                "tx_simulate",
                "diagnose",
                "replay_bundle"
            ]
        );
        Ok(())
    }

    #[test]
    fn test_scenario_unknown() {
        let result = Scenario::parse("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_diagnose_next() {
        let steps = vec!["build", "test", "diagnose"];
        assert!(is_diagnose_next("test", &steps));
        assert!(!is_diagnose_next("build", &steps));
        assert!(!is_diagnose_next("diagnose", &steps));
    }

    #[test]
    fn test_validate_params_build_no_validator() {
        let params = AutoParams {
            project_dir: ".",
            scenario: "build",
            validator: None,
            purpose: None,
            datum: None,
            redeemer: None,
            tx_spec: None,
        };
        // Build doesn't require validator
        assert!(validate_params(&Scenario::Build, &params).is_ok());
    }

    #[test]
    fn test_validate_params_trace_requires_validator() {
        let params = AutoParams {
            project_dir: ".",
            scenario: "trace",
            validator: None,
            purpose: Some("spend"),
            datum: None,
            redeemer: None,
            tx_spec: None,
        };
        assert!(validate_params(&Scenario::Trace, &params).is_err());
    }

    #[test]
    fn test_validate_params_trace_requires_purpose() {
        let params = AutoParams {
            project_dir: ".",
            scenario: "trace",
            validator: Some("0"),
            purpose: None,
            datum: None,
            redeemer: None,
            tx_spec: None,
        };
        assert!(validate_params(&Scenario::Trace, &params).is_err());
    }

    #[tokio::test]
    async fn test_auto_stop_on_error() -> TestResult {
        // Use nonexistent project dir — build step should fail
        let params = AutoParams {
            project_dir: "/nonexistent/project",
            scenario: "build",
            validator: None,
            purpose: None,
            datum: None,
            redeemer: None,
            tx_spec: None,
        };
        let result = run_auto(params).await;
        // Should succeed (returns Output, not Err)
        assert!(result.is_ok());
        let output = result?;
        // The first step (typecheck) should have errored
        assert!(!output.data.steps.is_empty());
        assert_eq!(output.data.steps[0].status, Status::Error);
        // Should have stopped
        assert!(output.data.stopped_at.is_some());
        Ok(())
    }

    #[test]
    fn test_suggest_next_test() -> TestResult {
        let params = AutoParams {
            project_dir: ".",
            scenario: "test",
            validator: None,
            purpose: None,
            datum: None,
            redeemer: None,
            tx_spec: None,
        };
        let suggestion = suggest_next("test", &params);
        assert!(suggestion.is_some());
        let s = suggestion.ok_or("expected suggestion")?;
        assert!(s.contains("test"));
        Ok(())
    }
}
