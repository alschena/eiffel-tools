use crate::code_entities::prelude::*;
use std::process::Output;
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

pub async fn verify_feature(
    class_name: &ClassName,
    feature_name: &str,
) -> Option<VerificationResult> {
    let output = autoproof(class_name, Some(feature_name)).await?;
    format_output(output).map(|message| verification_result(message))
}

pub async fn verify_class(class_name: &ClassName) -> Option<VerificationResult> {
    let output = autoproof(class_name, None).await?;
    format_output(output).map(|message| verification_result(message))
}

async fn autoproof(class_name: &ClassName, feature_name: Option<&str>) -> Option<Output> {
    let autoproof_cli = std::env::var("AP_COMMAND").inspect_err(
        |e| eprintln!("fails to find environment variable `AP_COMMAND` pointing to the AutoProof executable with error {:#?}", e),
    ).ok()?;

    let cli_args = {
        let upcase_classname = class_name.to_string().to_uppercase();
        feature_name.map_or_else(
            || format!("{}", upcase_classname),
            |feature_name| format!("{}.{}", upcase_classname, feature_name),
        )
    };

    tokio::process::Command::new(autoproof_cli)
        .arg("-autoproof")
        .arg(&cli_args)
        .output()
        .await
        .inspect_err(|e| {
            eprintln!(
                "fails to run the autoproof command `ec -autoproof {}` with error {:#?}",
                cli_args, e
            )
        })
        .ok()
}

fn format_output(autoproof_output: std::process::Output) -> Option<String> {
    fn log_failure_converting_to_utf8(error: &std::string::FromUtf8Error) {
        eprintln!(
            "fails to convert stdout from autoproof command to UTF-8 string with error: {:#?}",
            error
        )
    }

    let to_stdout = String::from_utf8(autoproof_output.stdout)
        .inspect_err(log_failure_converting_to_utf8)
        .ok()?;

    let to_stderr = String::from_utf8(autoproof_output.stderr)
        .inspect_err(log_failure_converting_to_utf8)
        .ok()?;

    if !to_stderr.is_empty() {
        info!(
            target: "llm",
            "AutProof counterexample goes into stderr: {:#?}",
            &to_stderr
        );
    }

    if !to_stdout.is_empty() {
        info!(
            target: "llm",
            "AutProof counterexample goes into stdout: {:#?}",
            &to_stdout
        );
    }

    Some(format!(
        r#"
    This is the counterexample AutoProof provides: 
    {}
    {}"#,
        to_stdout, to_stderr
    ))
}
