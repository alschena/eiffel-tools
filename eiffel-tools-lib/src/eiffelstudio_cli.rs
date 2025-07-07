use crate::code_entities::prelude::*;
use tracing::info;
use tracing::warn;

pub enum VerificationResult {
    Success,
    Failure(String),
}

fn verification_result(verification_message: String) -> VerificationResult {
    match verification_message {
        s if s.contains("Syntax error") => {
            info!(target: "autoproof", "AutoProof fails to parse because: {}", s);
            VerificationResult::Failure(s)
        }
        s if s.contains("Type error") => {
            info!(target: "autoproof", "AutoProof fails to type check because: {}", s);
            VerificationResult::Failure(s)
        }
        s if s.contains("Error code") => {
            info!(target: "autoproof", "AutoProof fails to compile because of the following error: {}", s);
            VerificationResult::Failure(s)
        }
        s if s.contains("Verification failed") => {
            info!(target: "autoproof", "AutoProof fails to verify because: {}", s);
            VerificationResult::Failure(s)
        }
        _ => {
            info!(target: "autoproof", "Autoproof succedes.");
            VerificationResult::Success
        }
    }
}

async fn autoproof(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
) -> Option<VerificationResult> {
    let autoproof_cli = std::env::var("AP_COMMAND").inspect_err(
        |e| warn!("fails to find environment variable `AP_COMMAND` pointing to the AutoProof executable with error {:#?}", e),
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
            warn!(
                "fails to run the autoproof command `ec -autoproof {}` with error {:#?}",
                cli_args, e
            )
        })
        .ok()
        .and_then(format_output)
        .map(|message| verification_result(message))
}

pub fn verify(
    class_name: ClassName,
    feature_name: Option<FeatureName>,
    max_secs: u64,
) -> tokio::task::JoinHandle<Result<Option<VerificationResult>, tokio::time::error::Elapsed>> {
    tokio::spawn(tokio::time::timeout(
        tokio::time::Duration::from_secs(max_secs),
        async move { autoproof(&class_name, feature_name.as_ref()).await },
    ))
}

fn format_output(autoproof_output: std::process::Output) -> Option<String> {
    fn log_failure_converting_to_utf8(error: &std::string::FromUtf8Error) {
        warn!(
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
            target: "autoproof",
            "AutProof counterexample goes into stderr: {:#?}",
            &to_stderr
        );
    }

    if !to_stdout.is_empty() {
        info!(
            target: "autoproof",
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
