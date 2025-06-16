use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::autoproof;
use std::path::Path;
use tracing::warn;

pub async fn failsafe_verification(
    path: &Path,
    number_of_tries: &mut usize,
    last_valid_code: &mut Vec<u8>,
    classname: ClassName,
    featurename: Option<FeatureName>,
) -> std::ops::ControlFlow<(), Option<VerificationResult>> {
    let verification_handle = tokio::spawn(tokio::time::timeout(
        tokio::time::Duration::from_secs(60),
        async move { autoproof(&classname, featurename.as_ref()).await },
    ))
    .await;

    match verification_handle {
        Ok(Ok(result)) => {
            last_valid_code.clone_from(
                &tokio::fs::read(&path)
                    .await
                    .unwrap_or_else(|e| panic!("fails to read {path:#?} because {e:#?}")),
            );

            std::ops::ControlFlow::Continue(result)
        }
        Ok(Err(_timeout_elapsed)) => {
            warn!(target: "autoproof", "AutoProof times out try #{number_of_tries} of {path:#?}.");
            tokio::fs::write(&path, &last_valid_code)
                .await
                .unwrap_or_else(|e| panic!("fails to read at path {path:#?} with {e:#?}"));
            *number_of_tries += 1;
            std::ops::ControlFlow::Break(())
        }
        Err(_fails_to_complete_task) => {
            warn!(
                target: "autoproof",
                "AutoProof fails either for the logic in the function `verify_class` or because of an internal processing error of AutoProof."
            );

            tokio::fs::write(&path, &last_valid_code)
                .await
                .unwrap_or_else(|e| panic!("fails to read {path:#?} because {e:#?}"));
            *number_of_tries += 1;
            std::ops::ControlFlow::Break(())
        }
    }
}
