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
    verbose: bool,
) -> tokio::task::JoinHandle<Result<Option<VerificationResult>, tokio::time::error::Elapsed>> {
    tokio::spawn(async move {
        let autoproof_cli = std::env::var("AP_COMMAND")
            .expect("AP_COMMAND environment variable must be set to the AutoProof executable path");

        let cli_args = {
            let upcase_classname = class_name.to_string().to_uppercase();
            feature_name.as_ref().map_or_else(
                || upcase_classname.to_string(),
                |feature_name| format!("{}.{}", upcase_classname, feature_name),
            )
        };

        // Build command string for error messages
        let command_string = format!("{} -batch -autoproof {}", autoproof_cli, cli_args);

        // Spawn the child process in its own process group so we can identify its PGID.
        // Grandchildren (boogie, Z3) create their own groups; we collect the full subtree
        // via /proc at timeout time and kill every process group in it.
        let mut cmd = tokio::process::Command::new(&autoproof_cli);
        cmd.arg("-batch")
            .arg("-autoproof")
            .arg(&cli_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child_opt = Some(match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                warn!(
                    "fails to spawn the autoproof command `{}` with error {:#?}",
                    command_string, e
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
                            "fails to wait for autoproof command `{}` with error {:#?}",
                            command_string, e
                        );
                        // Try to kill by PID if we have it
                        if let Some(pid) = child_pid {
                            kill_process_by_pid(pid, &command_string).await;
                        }
                        return Ok(None);
                    }
                };

                // Always try to kill the process by PID after wait_with_output completes
                // This handles EiffelStudio bugs where the process doesn't fully terminate
                if let Some(pid) = child_pid {
                    kill_process_by_pid(pid, &command_string).await;
                }

                let result = format_output(output, verbose);
                Ok(result.map(verification_result))
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(max_secs)) => {
                warn!(
                    target: "autoproof",
                    "AutoProof verification timeout after {} seconds for `{}`",
                    max_secs, command_string
                );

                // Collect the entire process subtree rooted at ecb BEFORE killing anything.
                // ecb → boogie → Z3: boogie creates its own process group, so killing only
                // ecb's PGID leaves boogie and Z3 alive.  Once ecb exits /proc/<pid>/ is gone,
                // so we must snapshot the tree now.
                #[cfg(unix)]
                let subtree_pids: Vec<u32> = child_pid
                    .map(collect_subtree_pids)
                    .unwrap_or_default();

                // Try to drain up to 1 MB of output before killing (best-effort).
                let mut stdout_bytes = Vec::new();
                let mut stderr_bytes = Vec::new();
                const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

                if let Some(mut child) = child_opt.take() {
                    if let Some(mut stdout) = child.stdout.take() {
                        use tokio::io::AsyncReadExt;
                        let mut buf = [0u8; 4096];
                        loop {
                            if stdout_bytes.len() >= MAX_OUTPUT_BYTES { break; }
                            match tokio::time::timeout(
                                tokio::time::Duration::from_millis(100),
                                stdout.read(&mut buf),
                            ).await {
                                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                                Ok(Ok(n)) => stdout_bytes.extend_from_slice(&buf[..n]),
                            }
                        }
                    }
                    if let Some(mut stderr) = child.stderr.take() {
                        use tokio::io::AsyncReadExt;
                        let mut buf = [0u8; 4096];
                        loop {
                            if stderr_bytes.len() >= MAX_OUTPUT_BYTES { break; }
                            match tokio::time::timeout(
                                tokio::time::Duration::from_millis(100),
                                stderr.read(&mut buf),
                            ).await {
                                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                                Ok(Ok(n)) => stderr_bytes.extend_from_slice(&buf[..n]),
                            }
                        }
                    }
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                } else if let Some(pid) = child_pid {
                    kill_process_by_pid(pid, &command_string).await;
                }

                // Kill the full subtree (ecb's PGID + every descendant process group).
                #[cfg(unix)]
                {
                    use std::process::Command;
                    for pid in &subtree_pids {
                        let _ = Command::new("kill").args(["-9", &format!("-{}", pid)]).output();
                        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                    }
                    info!(
                        target: "autoproof",
                        "Killed subtree ({} process(es)) for `{}`",
                        subtree_pids.len(), command_string
                    );
                }

                if verbose {
                    let label = "=== AutoProof Output (timeout) ===";
                    eprintln!("{label}");
                    if stdout_bytes.is_empty() && stderr_bytes.is_empty() {
                        eprintln!("No output captured before timeout.");
                    } else {
                        if !stdout_bytes.is_empty() {
                            eprintln!("stdout ({} bytes):", stdout_bytes.len());
                            eprintln!("{}", String::from_utf8_lossy(&stdout_bytes));
                        }
                        if !stderr_bytes.is_empty() {
                            eprintln!("stderr ({} bytes):", stderr_bytes.len());
                            eprintln!("{}", String::from_utf8_lossy(&stderr_bytes));
                        }
                    }
                    eprintln!("=== End AutoProof Output (timeout) ===");
                }

                // Return timeout error via a zero-duration timeout on a never-completing future
                // (Elapsed constructor is private, so we can't construct it directly).
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

/// Walk /proc to collect all PIDs in the subtree rooted at `root_pid`
/// (Linux only; requires /proc/<pid>/task/<tid>/children, available since kernel 3.5).
/// Must be called while `root_pid` is still alive — the tree disappears when it exits.
#[cfg(unix)]
fn collect_subtree_pids(root_pid: u32) -> Vec<u32> {
    let mut pids = vec![root_pid];
    let mut i = 0;
    while i < pids.len() {
        let pid = pids[i];
        if let Ok(task_dir) = std::fs::read_dir(format!("/proc/{}/task", pid)) {
            for entry in task_dir.flatten() {
                let children_path = entry.path().join("children");
                if let Ok(content) = std::fs::read_to_string(children_path) {
                    for s in content.split_whitespace() {
                        if let Ok(child_pid) = s.parse::<u32>() {
                            if !pids.contains(&child_pid) {
                                pids.push(child_pid);
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }
    pids
}

/// Kill a process and its process group.
/// Used for the completion path (after wait_with_output) to catch any processes
/// EiffelStudio leaves behind.  For the timeout path, kill_process_by_pid is called
/// only as a fallback; the subtree kill is done inline before ecb exits.
async fn kill_process_by_pid(pid: u32, command_string: &str) {
    #[cfg(unix)]
    {
        use std::process::Command;
        let _ = Command::new("kill").args(["-9", &format!("-{}", pid)]).output();
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
        info!(
            target: "autoproof",
            "Killed AutoProof process group -{} and process {} for `{}`",
            pid, pid, command_string
        );
    }
    #[cfg(not(unix))]
    {
        warn!(
            target: "autoproof",
            "Cannot kill process by PID on this platform for `{}`",
            command_string
        );
    }
}

fn format_output(autoproof_output: std::process::Output, verbose: bool) -> Option<String> {
    
    // Try to convert to UTF-8, but print output even if conversion fails
    let to_stdout = String::from_utf8(autoproof_output.stdout.clone())
        .inspect_err(|e| {
            warn!(
                "fails to convert stdout from autoproof command to UTF-8 string with error: {:#?}",
                e
            );
            // Print raw bytes as hex if UTF-8 conversion fails (only in verbose mode)
            if verbose {
                eprintln!("AutoProof stdout (raw bytes, UTF-8 conversion failed):\n{:?}", autoproof_output.stdout);
            }
        })
        .unwrap_or_else(|_| String::from(""));

    let to_stderr = String::from_utf8(autoproof_output.stderr.clone())
        .inspect_err(|e| {
            warn!(
                "fails to convert stderr from autoproof command to UTF-8 string with error: {:#?}",
                e
            );
            // Print raw bytes as hex if UTF-8 conversion fails (only in verbose mode)
            if verbose {
                eprintln!("AutoProof stderr (raw bytes, UTF-8 conversion failed):\n{:?}", autoproof_output.stderr);
            }
        })
        .unwrap_or_else(|_| String::from(""));

    // Print verification output to stderr if verbose
    if verbose {
        eprintln!("=== AutoProof Verification Output ===");
        eprintln!("AutoProof stdout ({} bytes):", autoproof_output.stdout.len());
        if to_stdout.is_empty() {
            eprintln!("(empty)");
        } else {
            eprintln!("{}", to_stdout);
        }
        eprintln!("AutoProof stderr ({} bytes):", autoproof_output.stderr.len());
        if to_stderr.is_empty() {
            eprintln!("(empty)");
        } else {
            eprintln!("{}", to_stderr);
        }
        eprintln!("=== End AutoProof Output ===");
    }

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
