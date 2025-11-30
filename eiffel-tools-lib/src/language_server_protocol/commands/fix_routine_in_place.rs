use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use serde::Serialize;
use std::ops::ControlFlow;
use tracing::info;
use tracing::instrument;

#[derive(Debug, Clone, Serialize)]
pub struct LlmInteraction {
    pub interaction_number: u32,
    pub error_message: String,
    pub generated_code: Option<String>,
    pub applied: bool,
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
}

#[instrument(skip_all)]
pub async fn fix_routine_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
    feature_name: &FeatureName,
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

    while let ControlFlow::Continue(verifier_failure_feedback) = modify_in_place::verification(
        class_name,
        Some(feature_name),
        workspace,
        &mut last_valid_code,
    )
    .await
    {
        number_of_tries += 1;
        if max_number_of_tries <= number_of_tries {
            info!(target: "autoproof", "Giving up on verifiying {class_name}.{}",feature_name);
            max_retries_reached = true;
            break;
        }
        info!(target:"autoproof", "Try #{number_of_tries} on {class_name}.{}",feature_name);
        if let Some(error_message) = verifier_failure_feedback {
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
            let llm_result = generators
                .fixed_routine_src(workspace, &path, feature_name, error_message.clone())
                .await;

            let (generated_code, applied) = if let Some((ft, body)) = llm_result {
                let generated = body.clone();
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

                // Record code change if it's different
                if before_code != after_code {
                    code_changes.push(CodeChange {
                        change_number: code_changes.len() as u32 + 1,
                        before_code: before_code.clone(),
                        after_code,
                    });
                }

                (Some(generated), true)
            } else {
                (None, false)
            };

            // Record LLM interaction
            interactions.push(LlmInteraction {
                interaction_number: llm_interactions,
                error_message: error_message.clone(),
                generated_code,
                applied,
            });
        }
    }

    // If we exited the loop without max retries, verification succeeded
    let final_status = if max_retries_reached {
        format!("Max retries ({}) reached", max_number_of_tries)
    } else {
        success = true;
        format!("Verification passed after {} LLM interaction(s)", llm_interactions)
    };

    FixRoutineResult {
        llm_interactions,
        success,
        max_retries_reached,
        final_status,
        interactions,
        code_changes,
    }
}
