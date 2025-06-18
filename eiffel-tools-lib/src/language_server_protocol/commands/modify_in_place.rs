use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::autoproof;
use crate::generators::Generators;
use crate::workspace::Workspace;
use std::error::Error;
use std::ops::ControlFlow::Break;
use std::ops::ControlFlow::Continue;
use std::path::Path;
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

fn timebound_verification(
    class_name: ClassName,
    feature_name: Option<FeatureName>,
) -> tokio::task::JoinHandle<Result<Option<VerificationResult>, tokio::time::error::Elapsed>> {
    tokio::spawn(tokio::time::timeout(
        tokio::time::Duration::from_secs(60),
        async move { autoproof(&class_name, feature_name.as_ref()).await },
    ))
}

pub async fn failsafe_verification(
    workspace: &mut Workspace,
    class_name: ClassName,
    feature_name: Option<FeatureName>,
    last_valid_code: &mut Vec<u8>,
) -> Result<VerificationResult, ModifyInPlaceErrors> {
    let verification_handle =
        timebound_verification(class_name.clone(), feature_name.clone()).await;

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
                class_name,
                feature_name,
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
