use super::modify_in_place;
use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use std::ops::ControlFlow;
use tracing::info;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn fix_routine_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
    feature_name: &FeatureName,
) {
    let path = workspace.path(class_name).to_path_buf();
    let mut last_valid_code = tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("fails to read at path {:#?} with {:#?}", &path, e));
    let max_number_of_tries = 10;
    let mut number_of_tries = 0;

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
            break;
        }
        info!(target:"autoproof", "Try #{number_of_tries} on {class_name}.{}",feature_name);
        if let Some(error_message) = verifier_failure_feedback {
            if let Some((ft, body)) = generators
                .fixed_routine_src(workspace, &path, feature_name, error_message)
                .await
            {
                modify_in_place::rewrite_features(&path, &[(ft.name().to_owned(), body)]).await;
            }
        }
    }
}
