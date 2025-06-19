use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use std::ops::ControlFlow;
use tracing::info;

pub async fn fix_routine_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
    feature: &Feature,
) {
    let path = workspace.path(class_name).to_path_buf();
    let mut last_valid_code = tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("fails to read at path {:#?} with {:#?}", &path, e));

    for number_of_tries in 0..10 {
        let feature_name = feature.name();

        let verification = modify_in_place::verification(
            class_name,
            Some(feature_name),
            workspace,
            &mut last_valid_code,
        )
        .await;

        match verification {
            ControlFlow::Continue(Some(error_message)) => {
                info!(target:"autoproof", "Fix #{number_of_tries} of {class_name}.{feature_name} fails.");
                if let Some((ft_name, body)) = generators
                    .routine_fixes(&workspace, &path, feature_name, error_message)
                    .await
                {
                    modify_in_place::rewrite_features(&path, &[(ft_name.to_owned(), body)]).await;
                }
            }
            ControlFlow::Continue(None) => {}
            ControlFlow::Break(_) => {
                break;
            }
        }
    }
}
