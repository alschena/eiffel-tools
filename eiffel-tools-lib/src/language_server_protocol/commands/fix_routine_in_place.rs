use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use serde::Serialize;
use std::ops::ControlFlow;
use std::time::Instant;
use tracing::info;
use tracing::instrument;
use tracing::warn;

#[derive(Debug, Clone, Serialize)]
pub struct LlmInteraction {
    pub interaction_number: u32,
    // The error message that triggered this fix (from verifying the before code)
    #[serde(rename = "error_message_before")]
    pub error_message_before: String,
    // The error message from verifying the generated code (or "Verification succeeded" if it passed)
    pub error_message: String,
    // The prompt sent to the LLM (system + user messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    // The raw LLM message/response (the full text response from the LLM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_message: Option<String>,
    pub applied: bool,
    #[serde(rename = "verification_time_seconds")]
    pub verification_time_seconds: f64,
    #[serde(rename = "ai_request_time_seconds")]
    pub ai_request_time_seconds: f64,
    // Code change information (if code was generated and applied)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_code: Option<String>,
    // Status/errors that occurred during code application (e.g., local clause extraction failures)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeChange {
    pub change_number: u32,
    pub before_code: String,
    pub after_code: String,
}

#[derive(Debug, Clone)]
pub struct FixRoutineResult {
    pub llm_interactions: u32,
    pub success: bool,
    pub max_retries_reached: bool,
    pub final_status: String,
    pub interactions: Vec<LlmInteraction>,
    pub code_changes: Vec<CodeChange>,
    pub total_elapsed_time_seconds: f64,
}

#[instrument(skip_all)]
pub async fn fix_routine_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
    feature_name: &FeatureName,
    verbose: bool,
) -> FixRoutineResult {
    let path = workspace.path(class_name).to_path_buf();
    let mut last_valid_code = tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("fails to read at path {:#?} with {:#?}", &path, e));
    // Save the original code to restore it if max retries are reached
    let original_code = last_valid_code.clone();
    let max_number_of_tries = 10;
    let mut number_of_tries = 0;
    let mut llm_interactions = 0;

    let mut success = false;
    let mut max_retries_reached = false;
    let mut interactions = Vec::new();
    let mut code_changes = Vec::new();
    let start_time = Instant::now();

    loop {
        number_of_tries += 1;
        if verbose {
            eprintln!("Starting attempt #{} for {}.{}", number_of_tries, class_name, feature_name);
        }
        
        if number_of_tries > max_number_of_tries {
            info!(target: "autoproof", "Giving up on verifiying {class_name}.{}",feature_name);
            max_retries_reached = true;
            break;
        }

        let verification_start = Instant::now();
        let verification_result = modify_in_place::verification(
            class_name,
            Some(feature_name),
            workspace,
            &mut last_valid_code,
            Some(number_of_tries),
            verbose,
        )
        .await;
        let verification_time = verification_start.elapsed().as_secs_f64();

        match verification_result {
            ControlFlow::Break(_) => {
                // Verification succeeded or failed to run
                break;
            }
            ControlFlow::Continue(verifier_failure_feedback) => {
                info!(target:"autoproof", "Try #{number_of_tries} on {class_name}.{}",feature_name);
                let error_message = verifier_failure_feedback.unwrap_or_else(|| {
                    String::from("Verification failed but no error message provided")
                });
                llm_interactions += 1;
                
                // Capture feature code before the change (full feature including local variables, contracts, etc.)
                let before_code = if let Some(class) = workspace.class(&path) {
                    if let Some(feature) = class.features().iter().find(|ft| ft.name() == feature_name) {
                        feature.source_unchecked(&path)
                            .await
                            .unwrap_or_else(|_| String::from("Unable to extract feature source"))
                    } else {
                        String::from("Feature not found")
                    }
                } else {
                    String::from("Class not found")
                };

                // Call LLM to generate fix
                let ai_request_start = Instant::now();
                let llm_result = generators
                    .fixed_routine_src(workspace, &path, feature_name, error_message.clone())
                    .await;
                let ai_request_time = ai_request_start.elapsed().as_secs_f64();

                let (llm_message, prompt, applied, after_code, verification_result_for_generated_code, application_status) = if let Some((ft, full_feature_source, raw_message, prompt_text)) = llm_result {
                    // Extract only the body from the LLM-generated feature to preserve original contracts
                    let body_only = ft.body_source_unchecked(full_feature_source.as_str())
                        .unwrap_or_else(|e| {
                            warn!(target: "llm", "Failed to extract body from LLM-generated feature, using full feature: {:#?}", e);
                            full_feature_source.clone()
                        });
                    // Extract local clause from LLM-generated feature (if present) using parser
                    let local_clause = ft.local_clause_source_unchecked(full_feature_source.as_str())
                        .ok()
                        .flatten();
                    if verbose {
                        eprintln!("[Attempt #{}] Applying code change to {}.{}", number_of_tries, class_name, feature_name);
                        if let Some(ref local) = local_clause {
                            eprintln!("[Attempt #{}] LLM suggested local clause: {}", number_of_tries, local);
                        }
                    }
                    // Apply both local clause (if present) and body, preserving contracts
                    let application_status = modify_in_place::rewrite_feature_bodies_and_locals(
                        &path,
                        &[(ft.name().to_owned(), body_only)],
                        &[(ft.name(), full_feature_source.as_str())],
                    ).await;
                    
                    // Reload workspace to get updated feature
                    workspace.reload(path.clone()).await;

                    // Save the generated code to file content before verification
                    // (verification may reset the file if it fails, so we need to restore it)
                    // This is the exact file content that will be sent to verification
                    let generated_code_file_content = tokio::fs::read(&path).await
                        .ok()
                        .unwrap_or_default();
                    
                    // Capture feature code after the change (full feature including local variables, contracts, etc.)
                    // This must be captured from the actual file content that will be sent to verification
                    // We use source_unchecked which reads from the file - this ensures we get the exact content
                    // that verification will see (read right before verification starts)
                    let after_code = if let Some(class) = workspace.class(&path) {
                        if let Some(feature) = class.features().iter().find(|ft| ft.name() == feature_name) {
                            // Read directly from file to ensure we get the exact content sent to verification
                            feature.source_unchecked(&path)
                                .await
                                .unwrap_or_else(|_| String::from("Unable to extract feature source"))
                        } else {
                            String::from("Feature not found")
                        }
                    } else {
                        String::from("Class not found")
                    };

                    if verbose {
                        if before_code != after_code {
                            eprintln!("[Attempt #{}] Code change for {}.{}:\nBEFORE:\n{}\nAFTER:\n{}", 
                                number_of_tries, class_name, feature_name, before_code, after_code);
                        } else {
                            eprintln!("[Attempt #{}] Code change for {}.{} (before and after are identical):\nBEFORE:\n{}\nAFTER:\n{}", 
                                number_of_tries, class_name, feature_name, before_code, after_code);
                        }
                    }
                    
                    // Update last_valid_code to the generated code so that if verification fails,
                    // the reset will keep the generated code (not the old code)
                    last_valid_code.clone_from(&generated_code_file_content);

                    // Verify the generated code to get the error message that matches it
                    let verification_start_for_generated = Instant::now();
                    let verification_result_for_generated = modify_in_place::verification(
                        class_name,
                        Some(feature_name),
                        workspace,
                        &mut last_valid_code,
                        Some(number_of_tries),
                        verbose,
                    )
                    .await;
                    let verification_time_for_generated = verification_start_for_generated.elapsed().as_secs_f64();

                    // Record code change for backward compatibility
                    let change_number = code_changes.len() as u32 + 1;
                    code_changes.push(CodeChange {
                        change_number,
                        before_code: before_code.clone(),
                        after_code: after_code.clone(),
                    });

                    let error_message_for_generated = match &verification_result_for_generated {
                        ControlFlow::Break(_) => {
                            // Verification succeeded - this will cause the loop to break
                            String::from("Verification succeeded")
                        }
                        ControlFlow::Continue(verifier_failure_feedback) => {
                            verifier_failure_feedback.clone().unwrap_or_else(|| {
                                String::from("Verification failed but no error message provided")
                            })
                        }
                    };

                    (Some(raw_message), Some(prompt_text), true, Some(after_code), Some((error_message_for_generated, verification_time_for_generated, verification_result_for_generated)), application_status)
                } else {
                    (None, None, false, None, None, None)
                };

                // Determine the error message to use: if code was generated and applied, use the verification result of that code
                let (final_error_message, final_verification_time, should_break) = if let Some((error_msg, verif_time, ref verif_result)) = verification_result_for_generated_code {
                    let should_break = matches!(verif_result, ControlFlow::Break(_));
                    (error_msg.clone(), verif_time, should_break)
                } else {
                    // If no code was generated/applied, use the original error message
                    (error_message.clone(), verification_time, false)
                };

                // Record LLM interaction with code change information
                // error_message_before: the error that triggered this fix (from verifying before_code)
                // error_message: the verification result of the generated code
                interactions.push(LlmInteraction {
                    interaction_number: llm_interactions,
                    error_message_before: error_message.clone(),
                    error_message: final_error_message,
                    prompt,
                    llm_message,
                    applied,
                    verification_time_seconds: final_verification_time,
                    ai_request_time_seconds: ai_request_time,
                    before_code: if applied { Some(before_code) } else { None },
                    after_code,
                    status: application_status,
                });

                // If the generated code was verified and succeeded, break the loop
                if should_break {
                    break;
                }
            }
        }
    }

    // If we exited the loop without max retries, verification succeeded
    let final_status = if max_retries_reached {
        // Rollback to original code when max retries are exhausted
        // This fix ensures code is restored to its original state after exhausting all attempts.
        // Before this fix, code would remain in its modified state after max retries were reached.
        tokio::fs::write(&path, &original_code)
            .await
            .unwrap_or_else(|e| panic!("fails to write original code at path {:#?} with {:#?}", &path, e));
        workspace.reload(path.clone()).await;
        format!("Max retries ({}) reached", max_number_of_tries)
    } else {
        success = true;
        format!("Verification passed after {} LLM interaction(s)", llm_interactions)
    };

    let total_elapsed_time = start_time.elapsed().as_secs_f64();

    FixRoutineResult {
        llm_interactions,
        success,
        max_retries_reached,
        final_status,
        interactions,
        code_changes,
        total_elapsed_time_seconds: total_elapsed_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    const INITIAL_CODE: &str = r#"
class TEST_CLASS
feature
    sum (n: INTEGER): INTEGER
        require
            n >= 0
        do
            if n = 0 then
                Result := 0
            else
                Result := n + sum(n - 1)
            end
        ensure
            result_correct: Result = n * (n + 1) // 2
        end
end
"#;

    const MODIFIED_CODE: &str = r#"
class TEST_CLASS
feature
    sum (n: INTEGER): INTEGER
        require
            n >= 0
        do
            if n = 0 then
                Result := 3  -- Buggy code
            else
                Result := n * (n + 1) + sum(n - 1)  -- Buggy code
            end
        ensure
            result_correct: Result = n * (n + 1) // 2
        end
end
"#;

    const GENERATED_CODE: &str = r#"
class TEST_CLASS
feature
    sum (n: INTEGER): INTEGER
        require
            n >= 0
        do
            if n = 0 then
                Result := 0  -- Fixed code
            else
                Result := n + sum(n - 1)  -- Fixed code
            end
        ensure
            result_correct: Result = n * (n + 1) // 2
        end
end
"#;

    #[tokio::test]
    async fn test_code_rollback_on_verification_failure() {
        // This test verifies that when verification fails after applying generated code,
        // the code is correctly rolled back to the last_valid_code (which should be the generated code)
        let tmp_dir = TempDir::new().expect("Failed to create temporary directory");
        let file = tmp_dir.child("test_class.e");

        // Write initial code
        file.write_str(INITIAL_CODE)
            .expect("Failed to write initial code");

        // Parse and create workspace
        let mut parser = Parser::default();
        let (class, tree) = parser
            .class_and_tree_from_source(INITIAL_CODE)
            .expect("Failed to parse initial class");
        let mut workspace = Workspace::new();
        workspace.add_file((class.clone(), file.to_path_buf(), tree));

        let _class_name = class.name();
        let _feature_name = FeatureName::from("sum".to_string());

        // Read initial code as last_valid_code
        let mut last_valid_code = tokio::fs::read(file.path())
            .await
            .expect("Failed to read initial file");

        // Simulate applying generated code: modify the file
        file.write_str(GENERATED_CODE)
            .expect("Failed to write generated code");
        workspace.reload(file.to_path_buf()).await;

        // Update last_valid_code to the generated code (this is what happens in fix_routine_in_place)
        let generated_code_bytes = tokio::fs::read(file.path())
            .await
            .expect("Failed to read generated code");
        last_valid_code.clone_from(&generated_code_bytes);

        // Verify the file contains the generated code
        let file_content_before_reset = tokio::fs::read_to_string(file.path())
            .await
            .expect("Failed to read file before reset");
        assert!(
            file_content_before_reset.contains("Result := 0  -- Fixed code"),
            "File should contain generated code before reset"
        );

        // Now simulate a verification failure by resetting the file to last_valid_code
        // This is what happens in modify_in_place::verification when verification fails
        tokio::fs::write(file.path(), &last_valid_code)
            .await
            .expect("Failed to write last_valid_code");
        workspace.reload(file.to_path_buf()).await;

        // Verify the file was rolled back to last_valid_code (which is the generated code)
        let file_content_after_reset = tokio::fs::read_to_string(file.path())
            .await
            .expect("Failed to read file after reset");
        
        // The file should still contain the generated code (not the initial code)
        // because last_valid_code was updated to the generated code
        assert!(
            file_content_after_reset.contains("Result := 0  -- Fixed code"),
            "File should still contain generated code after reset (not rolled back to initial)"
        );
        assert!(
            !file_content_after_reset.contains("Result := 3  -- Buggy code"),
            "File should not contain the buggy modified code"
        );
        assert!(
            !file_content_after_reset.contains("Result := 0\n\t\telse\n\t\t\tResult := n + sum(n - 1)") || 
            file_content_after_reset.contains("Result := 0  -- Fixed code"),
            "File should contain the fixed code, not the initial code"
        );
    }

    #[tokio::test]
    async fn test_code_rollback_to_original_when_last_valid_not_updated() {
        // This test verifies that if last_valid_code is NOT updated before verification fails,
        // the code is rolled back to the original code
        let tmp_dir = TempDir::new().expect("Failed to create temporary directory");
        let file = tmp_dir.child("test_class.e");

        // Write initial code
        file.write_str(INITIAL_CODE)
            .expect("Failed to write initial code");

        // Parse and create workspace
        let mut parser = Parser::default();
        let (class, tree) = parser
            .class_and_tree_from_source(INITIAL_CODE)
            .expect("Failed to parse initial class");
        let mut workspace = Workspace::new();
        workspace.add_file((class.clone(), file.to_path_buf(), tree));

        // Read initial code as last_valid_code (don't update it)
        let last_valid_code = tokio::fs::read(file.path())
            .await
            .expect("Failed to read initial file");

        // Modify the file to buggy code
        file.write_str(MODIFIED_CODE)
            .expect("Failed to write modified code");
        workspace.reload(file.to_path_buf()).await;

        // Verify the file contains the modified code
        let file_content_before_reset = tokio::fs::read_to_string(file.path())
            .await
            .expect("Failed to read file before reset");
        assert!(
            file_content_before_reset.contains("Result := 3  -- Buggy code"),
            "File should contain modified code before reset"
        );

        // Simulate verification failure: reset the file to last_valid_code
        // Since last_valid_code was NOT updated, it should roll back to INITIAL_CODE
        tokio::fs::write(file.path(), &last_valid_code)
            .await
            .expect("Failed to write last_valid_code");
        workspace.reload(file.to_path_buf()).await;

        // Verify the file was rolled back to last_valid_code (which is the initial code)
        let file_content_after_reset = tokio::fs::read_to_string(file.path())
            .await
            .expect("Failed to read file after reset");
        
        // The file should contain the initial code (not the modified code)
        assert!(
            file_content_after_reset.contains("Result := 0") && 
            !file_content_after_reset.contains("Result := 3"),
            "File should contain initial code (Result := 0) and not buggy code (Result := 3) after reset. Content: {}",
            file_content_after_reset
        );
        assert!(
            !file_content_after_reset.contains("Result := 3  -- Buggy code"),
            "File should not contain the buggy modified code after reset"
        );
    }
}
