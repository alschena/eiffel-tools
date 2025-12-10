use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use serde::Serialize;
use std::ops::ControlFlow;
use std::time::Instant;
use tracing::info;
use tracing::instrument;

#[derive(Debug, Clone, Serialize)]
pub struct LlmInteraction {
    pub interaction_number: u32,
    pub error_message: String,
    pub generated_code: Option<String>,
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
                
                // Capture feature code before the change
                let before_code = if let Some(class) = workspace.class(&path) {
                    if let Some(feature) = class.features().iter().find(|ft| ft.name() == feature_name) {
                        let file_content = tokio::fs::read(&path).await
                            .ok()
                            .and_then(|content| String::from_utf8(content).ok());
                        if let Some(file_str) = file_content {
                            feature.body_source_unchecked(file_str.as_str())
                                .unwrap_or_else(|_| String::from("Unable to extract feature body"))
                        } else {
                            String::from("Unable to read file")
                        }
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

                let (generated_code, applied, after_code) = if let Some((ft, body)) = llm_result {
                    let generated = body.clone();
                    if verbose {
                        eprintln!("[Attempt #{}] Applying code change to {}.{}", number_of_tries, class_name, feature_name);
                    }
                    modify_in_place::rewrite_features(&path, &[(ft.name().to_owned(), body)]).await;
                    
                    // Reload workspace to get updated feature
                    workspace.reload(path.clone()).await;
                    
                    // Capture feature code after the change
                    let after_code = if let Some(class) = workspace.class(&path) {
                        if let Some(feature) = class.features().iter().find(|ft| ft.name() == feature_name) {
                            let file_content = tokio::fs::read(&path).await
                                .ok()
                                .and_then(|content| String::from_utf8(content).ok());
                            if let Some(file_str) = file_content {
                                feature.body_source_unchecked(file_str.as_str())
                                    .unwrap_or_else(|_| String::from("Unable to extract feature body"))
                            } else {
                                String::from("Unable to read file")
                            }
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

                    // Record code change for backward compatibility
                    let change_number = code_changes.len() as u32 + 1;
                    code_changes.push(CodeChange {
                        change_number,
                        before_code: before_code.clone(),
                        after_code: after_code.clone(),
                    });

                    (Some(generated), true, Some(after_code))
                } else {
                    (None, false, None)
                };

                // Record LLM interaction with code change information
                interactions.push(LlmInteraction {
                    interaction_number: llm_interactions,
                    error_message: error_message.clone(),
                    generated_code,
                    applied,
                    verification_time_seconds: verification_time,
                    ai_request_time_seconds: ai_request_time,
                    before_code: if applied { Some(before_code) } else { None },
                    after_code,
                });
            }
        }
    }

    // If we exited the loop without max retries, verification succeeded
    let final_status = if max_retries_reached {
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
