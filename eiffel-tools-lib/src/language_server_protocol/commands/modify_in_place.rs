use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify;
use crate::workspace::Workspace;
use std::error::Error;
use std::ops::ControlFlow;
use std::path::PathBuf;
use tracing::info;
use tracing::warn;

pub enum ModifyInPlaceErrors {
    TimeoutAutoProof {
        class_name: ClassName,
        feature_name: Option<FeatureName>,
        error: Box<dyn Error>,
    },
    TaskJoinError {
        error: Box<dyn Error>,
    },
    RunAutoProofCommand,
}

impl ModifyInPlaceErrors {
    pub fn log(&self) {
        match self {
            ModifyInPlaceErrors::TimeoutAutoProof {
                class_name,
                feature_name,
                ..
            } => {
                let name_entity_under_verification = feature_name.as_ref().map_or_else(
                    || format!("{class_name}"),
                    |name| format!("{class_name}.{name}"),
                );
                warn!(target: "autoproof", "AutoProof times out verifying {name_entity_under_verification}.");
            }
            ModifyInPlaceErrors::TaskJoinError { error } => {
                warn!("Fails to await for AutoProof task because {error:#?}");
            }
            ModifyInPlaceErrors::RunAutoProofCommand => {
                warn!("The LSP fails to run the AutoProof CLI.");
            }
        }
    }
}

async fn update_last_valid_source(
    workspace: &mut Workspace,
    path: PathBuf,
    last_valid_code: &mut Vec<u8>,
) {
    last_valid_code.clone_from(
        &tokio::fs::read(&path)
            .await
            .unwrap_or_else(|e| panic!("Fails to read {path:#?} because {e:#?}.")),
    );
    workspace.reload(path).await
}

async fn reset_source(workspace: &mut Workspace, path: PathBuf, last_valid_code: &mut Vec<u8>) {
    tokio::fs::write(&path, &last_valid_code)
        .await
        .unwrap_or_else(|e| panic!("Fails to read at path {path:#?} because {e:#?}."));
    workspace.reload(path).await
}

async fn failsafe_verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
) -> Result<VerificationResult, ModifyInPlaceErrors> {
    let verification_handle = verify(class_name.clone(), feature_name.cloned(), 60).await;

    let path = workspace.path(&class_name);
    match verification_handle {
        Ok(Ok(Some(verification_result))) => {
            update_last_valid_source(workspace, path.to_path_buf(), last_valid_code).await;
            Ok(verification_result)
        }
        Ok(Ok(None)) => Err(ModifyInPlaceErrors::RunAutoProofCommand),
        Ok(Err(timeout)) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            Err(ModifyInPlaceErrors::TimeoutAutoProof {
                class_name: class_name.clone(),
                feature_name: feature_name.cloned(),
                error: Box::new(timeout),
            })
        }
        Err(fails_to_complete_task) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            Err(ModifyInPlaceErrors::TaskJoinError {
                error: Box::new(fails_to_complete_task),
            })
        }
    }
}

pub async fn verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
) -> ControlFlow<(), Option<String>> {
    let verification =
        failsafe_verification(class_name, feature_name, workspace, last_valid_code).await;
    match verification {
        Ok(VerificationResult::Success) => {
            let success_message = feature_name.map_or_else(
                || format!("The class {class_name} verifies successfully."),
                |name| format!("The feature {class_name}.{name} verifies successfully."),
            );
            info!("{success_message}");

            ControlFlow::Break(())
        }
        Ok(VerificationResult::Failure(error_message)) => {
            ControlFlow::Continue(Some(error_message))
        }
        Err(e) => {
            e.log();
            ControlFlow::Continue(None)
        }
    }
}
