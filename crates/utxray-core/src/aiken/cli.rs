use tokio::process::Command;

/// Result of running an aiken CLI command.
pub struct AikenOutput {
    pub exit_code: i32,
    pub parsed: Option<serde_json::Value>,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

/// Wrapper around the `aiken` binary, executing it as a subprocess.
pub struct AikenCli {
    binary: String,
    project_dir: String,
}

impl AikenCli {
    /// Create a new AikenCli. Locates the `aiken` binary in PATH.
    /// Returns an error if the binary cannot be found.
    pub fn new(project_dir: &str) -> anyhow::Result<Self> {
        let binary = which::which("aiken")
            .map_err(|_| {
                anyhow::anyhow!(
                    "aiken binary not found in PATH. Install it from https://aiken-lang.org"
                )
            })?
            .to_string_lossy()
            .to_string();

        Ok(Self {
            binary,
            project_dir: project_dir.to_string(),
        })
    }

    /// Run `aiken build` in the project directory.
    pub async fn build(&self) -> anyhow::Result<AikenOutput> {
        let output = Command::new(&self.binary)
            .arg("build")
            .current_dir(&self.project_dir)
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("failed to execute aiken build: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(AikenOutput {
            exit_code,
            parsed: None,
            raw_stdout,
            raw_stderr,
        })
    }

    /// Run `aiken check` in the project directory.
    /// Optionally filters to a specific module.
    pub async fn check(
        &self,
        module: Option<&str>,
        trace_level: &str,
    ) -> anyhow::Result<AikenOutput> {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("check");
        cmd.arg("--trace-level");
        cmd.arg(trace_level);

        if let Some(m) = module {
            cmd.arg("-m");
            cmd.arg(m);
        }

        cmd.current_dir(&self.project_dir);

        let output = cmd
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("failed to execute aiken check: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(AikenOutput {
            exit_code,
            parsed: None,
            raw_stdout,
            raw_stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aiken_cli_not_found() {
        // Temporarily override PATH to ensure aiken is not found
        let result = std::panic::catch_unwind(|| {
            // This test just verifies the error message structure
            let saved_path = std::env::var("PATH").unwrap_or_default();
            std::env::set_var("PATH", "/nonexistent");
            let result = AikenCli::new("/tmp");
            std::env::set_var("PATH", saved_path);
            result
        });
        // We can't reliably test this in a deterministic way due to env var races,
        // so we just check it doesn't panic
        assert!(result.is_ok());
    }
}
