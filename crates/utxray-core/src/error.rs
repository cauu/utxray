use serde::Serialize;

/// Error codes from the spec's error code enumeration
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    #[error("type mismatch")]
    TypeMismatch,
    #[error("schema mismatch")]
    SchemaMismatch,
    #[error("constructor index wrong")]
    ConstructorIndexWrong,
    #[error("redeemer index mismatch")]
    RedeemerIndexMismatch,
    #[error("script data hash mismatch")]
    ScriptDataHashMismatch,
    #[error("datum not found")]
    DatumNotFound,
    #[error("utxo consumed")]
    UtxoConsumed,
    #[error("validity interval fail")]
    ValidityIntervalFail,
    #[error("phase 1 balance error")]
    Phase1BalanceError,
    #[error("phase 1 min utxo fail")]
    Phase1MinUtxoFail,
    #[error("phase 1 collateral missing")]
    Phase1CollateralMissing,
    #[error("phase 1 required signer missing")]
    Phase1RequiredSignerMissing,
    #[error("phase 1 tx size exceeded")]
    Phase1TxSizeExceeded,
    #[error("phase 2 script fail")]
    Phase2ScriptFail,
    #[error("phase 2 budget exceeded")]
    Phase2BudgetExceeded,
    #[error("phase 2 script error")]
    Phase2ScriptError,
    #[error("mint policy fail")]
    MintPolicyFail,
    #[error("withdrawal script fail")]
    WithdrawalScriptFail,
    #[error("cert script fail")]
    CertScriptFail,
    #[error("submit already spent")]
    SubmitAlreadySpent,
    #[error("submit network error")]
    SubmitNetworkError,
    #[error("mainnet safety block")]
    MainnetSafetyBlock,
    #[error("unknown error")]
    UnknownError,
}

/// Severity levels for diagnostics and warnings
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

/// Confidence levels for diagnostic results
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// Source of ExUnits budget data
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetSource {
    TraceMinimal,
    TraceFull,
    TxEvaluate,
    TxSimulate,
    Test,
}
