use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use std::ops::ControlFlow;
use tracing::info;

pub async fn fix_class_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
) {
    let path = workspace.path(class_name).to_path_buf();
    let mut last_valid_code = tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("fails to read at path {path:#?} with {e:#?}"));

    for number_of_tries in 0..10 {
        let verification =
            modify_in_place::verification(class_name, None, workspace, &mut last_valid_code).await;

        match verification {
            ControlFlow::Break(_) => {
                break;
            }
            ControlFlow::Continue(Some(error_message)) => {
                info!("Try #{number_of_tries} fails to fix {class_name}");

                let feature_candidates = generators
                    .class_wide_fixes(workspace, &path, error_message)
                    .await;

                modify_in_place::rewrite_features(workspace.path(class_name), &feature_candidates)
                    .await;
            }
            ControlFlow::Continue(None) => {}
        }
    }
}
