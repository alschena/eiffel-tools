use crate::code_entities::prelude::*;
use tracing::info;
use tracing::warn;

pub enum VerificationResult {
    Success,
    Failure(String),
}

fn verification_result(verification_message: String) -> VerificationResult {
    match verification_message {
        s if s.contains("system execution failed") => {
            info!(target: "autoproof", "EiffelStudio crashes because: {}", s);
            VerificationResult::Failure(s)
        }
        s if s.contains("AutoProof error") => {
            info!(target: "autoproof", "AutoProof fails due to an internal error: {}", s);
            VerificationResult::Failure(s)
        }
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

pub fn verify(
    class_name: ClassName,
    feature_name: Option<FeatureName>,
    max_secs: u64,
) -> tokio::task::JoinHandle<Result<Option<VerificationResult>, tokio::time::error::Elapsed>> {
    tokio::spawn(async move {
        let autoproof_cli = std::env::var("AP_COMMAND").inspect_err(
            |e| warn!("fails to find environment variable `AP_COMMAND` pointing to the AutoProof executable with error {:#?}", e),
        ).ok();

        let cli_args = if let Some(ref _autoproof_cli) = autoproof_cli {
            let upcase_classname = class_name.to_string().to_uppercase();
            feature_name.as_ref().map_or_else(
                || upcase_classname.to_string(),
                |feature_name| format!("{}.{}", upcase_classname, feature_name),
            )
        } else {
            return Ok(None);
        };

        let autoproof_cli = autoproof_cli.unwrap();

        // Spawn the child process
        let mut child_opt = Some(match tokio::process::Command::new(&autoproof_cli)
            .arg("-autoproof")
            .arg(&cli_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                warn!(
                    "fails to spawn the autoproof command `ec -autoproof {}` with error {:#?}",
                    cli_args, e
                );
                return Ok(None);
            }
        });

        // Get the process ID before we move child into wait_with_output
        // This allows us to kill the process by PID even after wait_with_output completes
        let child_pid = child_opt.as_ref().and_then(|c| c.id());

        // Use tokio::select to race between process completion and timeout
        // This ensures we can kill the child process when timeout occurs
        let result = tokio::select! {
            output_result = async {
                if let Some(child) = child_opt.take() {
                    child.wait_with_output().await
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "Child already taken"))
                }
            } => {
                // Process completed before timeout
                // Even though wait_with_output completed, we still need to ensure the process is killed
                // due to EiffelStudio bugs that may leave child processes running
                let output = match output_result {
                    Ok(output) => output,
                    Err(e) => {
                        warn!(
                            "fails to wait for autoproof command `ec -autoproof {}` with error {:#?}",
                            cli_args, e
                        );
                        // Try to kill by PID if we have it
                        if let Some(pid) = child_pid {
                            kill_process_by_pid(pid, &cli_args).await;
                        }
                        return Ok(None);
                    }
                };

                // Always try to kill the process by PID after wait_with_output completes
                // This handles EiffelStudio bugs where the process doesn't fully terminate
                if let Some(pid) = child_pid {
                    kill_process_by_pid(pid, &cli_args).await;
                }

                Ok(format_output(output).map(verification_result))
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(max_secs)) => {
                // Timeout occurred - kill the child process
                warn!(
                    target: "autoproof",
                    "AutoProof verification timeout after {} seconds for `ec -autoproof {}`",
                    max_secs, cli_args
                );
                
                if let Some(mut child) = child_opt.take() {
                    if let Err(e) = child.kill().await {
                        warn!(
                            target: "autoproof",
                            "Failed to kill AutoProof child process after timeout for `ec -autoproof {}`: {:#?}",
                            cli_args, e
                        );
                    } else {
                        info!(
                            target: "autoproof",
                            "Killed AutoProof child process after timeout for `ec -autoproof {}`",
                            cli_args
                        );
                    }
                    
                    // Wait for the process to actually terminate
                    let _ = child.wait().await;
                } else if let Some(pid) = child_pid {
                    // Child was already taken (shouldn't happen), but try to kill by PID anyway
                    kill_process_by_pid(pid, &cli_args).await;
                }
                
                // Return timeout error by using timeout on a never-completing future
                // This is a workaround since Elapsed constructor is private
                tokio::time::timeout(
                    tokio::time::Duration::from_secs(0),
                    std::future::pending::<Option<VerificationResult>>(),
                )
                .await
            }
        };

        result
    })
}

/// Kill a process by PID (platform-specific)
async fn kill_process_by_pid(pid: u32, cli_args: &str) {
    #[cfg(unix)]
    {
        use std::process::Command;
        // Try to kill the process and its children
        let _ = Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .output();
        info!(
            target: "autoproof",
            "Attempted to kill AutoProof process {} (and children) for `ec -autoproof {}`",
            pid, cli_args
        );
    }
    #[cfg(not(unix))]
    {
        // On non-Unix systems, we can't easily kill by PID
        // The child handle should have been used instead
        warn!(
            target: "autoproof",
            "Cannot kill process by PID on this platform for `ec -autoproof {}`",
            cli_args
        );
    }
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
