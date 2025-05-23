use crate::lib::code_entities::prelude::*;
use anyhow::Context;
use anyhow::Result;
use tracing::info;
use tracing::warn;

pub enum VerificationResult {
    Success,
    Failure(String),
}

pub fn autoproof(feature_name: &str, class_name: &ClassName) -> Result<VerificationResult> {
    let upcase_class_name = class_name.to_string().to_uppercase();

    let autoproof_cli = std::env::var("AP_COMMAND").with_context(|| {
        "fails to find environment variable `AP_COMMAND` pointing to the AutoProof executable."
    })?;

    let autoproof = std::process::Command::new(autoproof_cli)
        .arg("-autoproof")
        .arg(format!("{}.{}", upcase_class_name, feature_name))
        .output()
        .with_context(|| {
            format!(
                "fails to run the autoproof command: `ec -autoproof {}.{}`",
                upcase_class_name, feature_name
            )
        })?;

    let stderr_autoproof = String::from_utf8(autoproof.stderr)?;
    let stdout_autoproof = String::from_utf8(autoproof.stdout)?;

    if !stderr_autoproof.is_empty() {
        info!(
            target: "llm",
            "AutProof counterexample goes into stderr: {:#?}",
            &stderr_autoproof
        );
    }

    if !stdout_autoproof.is_empty() {
        info!(
            target: "llm",
            "AutProof counterexample goes into stdout: {:#?}",
            &stdout_autoproof
        );
    }

    let prefix = "\nThis is the counterexample AutoProof provides: ";

    let message = format!("{prefix}\nstdout:\t{stdout_autoproof}\nstderr:\t{stderr_autoproof}");

    warn!(target:"llm", "AutoProof failure message: {}", message);

    Ok(VerificationResult::Failure(message))
}
