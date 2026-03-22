use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::cbor::schema::{self, SchemaErrorData, SchemaValidateError};
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum SchemaCommands {
    /// Validate datum/redeemer against blueprint schema
    Validate {
        /// Validator name or index (e.g. "escrow.escrow.spend" or "0")
        #[arg(long)]
        validator: String,

        /// Purpose: spend, mint, withdrawal, certificate, propose, vote (aliases: withdraw, cert)
        #[arg(long)]
        purpose: String,

        /// Datum as inline JSON or file path (optional for non-spend purposes)
        #[arg(long)]
        datum: Option<String>,

        /// Redeemer as inline JSON or file path
        #[arg(long)]
        redeemer: String,
    },
}

pub async fn handle(cmd: SchemaCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        SchemaCommands::Validate {
            validator,
            purpose,
            datum,
            redeemer,
        } => handle_validate(
            &ctx.project,
            &validator,
            &purpose,
            datum.as_deref(),
            &redeemer,
            &ctx.format,
        ),
    }
}

fn handle_validate(
    project: &str,
    validator: &str,
    purpose: &str,
    datum: Option<&str>,
    redeemer: &str,
    format: &str,
) -> anyhow::Result<()> {
    match schema::validate_schema(project, validator, purpose, datum, redeemer) {
        Ok(output) => {
            print_output_formatted(&output, format)?;
        }
        Err(e) => {
            let (error_code, message) = match &e {
                SchemaValidateError::BlueprintNotFound(path) => (
                    "BLUEPRINT_NOT_FOUND".to_string(),
                    format!("Blueprint file not found: {path}"),
                ),
                SchemaValidateError::ValidatorNotFound(name) => (
                    "VALIDATOR_NOT_FOUND".to_string(),
                    format!("Validator not found in blueprint: {name}"),
                ),
                SchemaValidateError::InvalidJson(detail) => (
                    "INVALID_JSON".to_string(),
                    format!("Invalid JSON input: {detail}"),
                ),
                SchemaValidateError::BlueprintParse(detail) => (
                    "BLUEPRINT_PARSE_ERROR".to_string(),
                    format!("Failed to parse blueprint: {detail}"),
                ),
                SchemaValidateError::DatumRequired => (
                    "DATUM_REQUIRED".to_string(),
                    "Datum is required by schema but was not provided".to_string(),
                ),
                SchemaValidateError::RedeemerRequired => (
                    "REDEEMER_REQUIRED".to_string(),
                    "Redeemer is required but was not provided".to_string(),
                ),
                SchemaValidateError::Io(io_err) => {
                    ("IO_ERROR".to_string(), format!("IO error: {io_err}"))
                }
            };
            let output = Output::error(SchemaErrorData {
                error_code,
                message,
            });
            print_output_formatted(&output, format)?;
        }
    }
    Ok(())
}
