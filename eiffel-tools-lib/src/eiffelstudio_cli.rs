use crate::code_entities::prelude::*;
use anyhow::Context;
use anyhow::Result;
use tracing::info;

pub enum VerificationResult {
    Success,
    Failure(String),
}

fn verification_result(verification_message: String) -> VerificationResult {
    if verification_message.contains("Verification failed") {
        info!(target:"llm", "AutoProof fails with message: {}", verification_message);
        eprintln!("AutoProof fails with message: {}", verification_message);
        VerificationResult::Failure(verification_message)
    } else {
        info!(target: "llm",
        "Autoproof succedes.");
        eprintln!("AutoProof succedes with message: {}", verification_message);
        VerificationResult::Success
    }
}

pub async fn autoproof(feature_name: &str, class_name: &ClassName) -> Result<VerificationResult> {
    let upcase_class_name = class_name.to_string().to_uppercase();

    let autoproof_cli = std::env::var("AP_COMMAND").with_context(
        || "fails to find environment variable `AP_COMMAND` pointing to the AutoProof executable.",
    )?;

    let autoproof = tokio::process::Command::new(autoproof_cli)
        .arg("-autoproof")
        .arg(format!("{}.{}", upcase_class_name, feature_name))
        .output()
        .await
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

    let message = format!(
        r#"
This is the counterexample AutoProof provides: 
stdout:\t{stdout_autoproof}
stderr:\t{stderr_autoproof}"#
    );

    Ok(verification_result(message))
}
