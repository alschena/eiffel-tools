use crate::lib::code_entities::prelude::*;
use crate::lib::eiffel_source::EiffelSource;
use crate::lib::eiffelstudio_cli::autoproof;
use crate::lib::eiffelstudio_cli::VerificationResult;
use crate::lib::parser::Parser;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use contract::RoutineSpecification;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

mod post_processing;
mod prompt;

mod constructor_api;

#[derive(Debug, Default)]
pub struct Generators {
    llms: Vec<Arc<constructor_api::LLM>>,
}
impl Generators {
    pub async fn add_new(&mut self) {
        let Ok(llm) = constructor_api::LLM::try_new().await else {
            info!("fail to create LLM via constructor API");
            return;
        };
        self.llms.push(Arc::new(llm));
    }

    pub async fn more_routine_specifications(
        &self,
        feature: &Feature,
        workspace: &Workspace,
        path: &Path,
    ) -> Result<Vec<RoutineSpecification>> {
        let current_class = workspace
            .class(path)
            .with_context(|| format!("fails to find class loaded from path: {:#?}", path))?;

        let current_class_name = current_class.name();
        let current_class_model = current_class_name.model_extended(workspace.system_classes());

        let prompt = prompt::Prompt::feature_specification(
            feature,
            current_class_name,
            &current_class_model,
            path,
            workspace.system_classes(),
        )
        .await?;

        // Generate feature with specifications
        let completion_parameters = constructor_api::CompletionParameters {
            messages: prompt.to_messages(),
            n: Some(50),
            ..Default::default()
        };

        info!("{completion_parameters:#?}");

        let mut tasks = tokio::task::JoinSet::new();
        for llm in self.llms.iter().cloned() {
            let completion_parameters = completion_parameters.clone();
            tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
        }
        let completion_response = tasks.join_all().await;

        let completion_response_processed = completion_response
            .iter()
            .filter_map(|rs| match rs {
                Err(e) => {
                    info!("An LLM request has returned the error: {e:#?}");
                    None
                }
                Ok(reply) => Some(reply),
            })
            .flat_map(|reply| {
                reply.contents().filter_map(|candidate| {
                    info!("candidate:\t{candidate}");
                    let mut parser = Parser::new();
                    parser.feature_from_source(candidate).map_or_else(
                        |e| {
                            info!("fail to parse generated output with error: {e:#?}");
                            None
                        },
                        |ft| Some(ft.routine_specification()),
                    )
                })
            })
            .collect();
        info!("completions:\t{completion_response_processed:#?}");

        Ok(completion_response_processed)
    }

    pub async fn fix_routine(
        &self,
        workspace: &Workspace,
        path: &Path,
        feature: &Feature,
    ) -> Result<Option<String>> {
        let class = workspace
            .class(path)
            .ok_or_else(|| anyhow!("fails to find loaded class at path: {:#?}", path))?;

        let feature_body = feature.body_source_unchecked(path).await?;

        // Write subclass redefining `feature` copy-pasting the body
        tokio::fs::write(
            path_llm_feature_redefinition(path),
            candidate_body_in_subclass(
                class.name(),
                &name_subclass(class.name()),
                feature,
                feature_body,
            ),
        )
        .await?;

        let mut number_of_tries = 0;
        while let VerificationResult::Failure(error_message) =
            autoproof(feature.name(), &name_subclass(class.name()))?
        {
            if number_of_tries <= 5 {
                number_of_tries += 1;
            } else {
                break;
            }

            let prompt = prompt::Prompt::feature_fixes(feature, path, error_message)
                .await?
                .to_messages();

            info!(target: "llm", "prompt: {:#?}", prompt);

            // Generate feature with specifications
            let completion_parameters = constructor_api::CompletionParameters {
                messages: prompt,
                n: Some(5),
                ..Default::default()
            };

            info!(target: "llm", "completion parameters: {:#?}", completion_parameters);

            let mut tasks = tokio::task::JoinSet::new();
            for llm in self.llms.iter().cloned() {
                let completion_parameters = completion_parameters.clone();
                tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
            }
            let completion_response = tasks.join_all().await;

            let completion_response_processed: Option<String> = completion_response
                .iter()
                .filter_map(|rs| {
                    rs.as_ref()
                        .inspect_err(|e| {
                            warn!(
                                target = "llm",
                                "An LLM processing the feature fix has returned the error: {:#?}",
                                e
                            )
                        })
                        .ok()
                })
                .flat_map(|reply| reply.contents())
                .inspect(|candidate| {
                    info!(target: "llm", "candidate:\t{candidate}");
                })
                .filter_map(|candidate| {
                    let mut parser = Parser::new();

                    parser
                        .feature_from_source(candidate)
                        .inspect_err(|e| {
                            info!(target: "llm", "fail to parse LLM generated feature with error: {e:#?}");
                        })
                        .ok()?;

                    Some(candidate.to_owned())
                })
                .inspect(|filtered_candidate| {
                    info!(target: "llm", "filtered candidate:\t{:#?}", filtered_candidate);
                })
                .next();

            // Write text edits to disk.
            if let Some(candidate) = completion_response_processed {
                tokio::fs::write(
                    path_llm_feature_redefinition(path),
                    candidate_body_in_subclass(
                        class.name(),
                        &name_subclass(class.name()),
                        feature,
                        candidate,
                    ),
                )
                .await?;
            }
        }

        if let VerificationResult::Success =
            autoproof(feature.name(), &name_subclass(class.name()))?
        {
            warn!(
                target: "llm",
                "must copy body of generated redefinition in initial feature.");
        }

        Ok(Some(String::new()))
    }
}

fn path_llm_feature_redefinition(path: &Path) -> PathBuf {
    let Some(stem) = path.file_stem() else {
        panic!("fails to get file stem (filename without extension) of current file.")
    };

    let Some(stem) = stem.to_str() else {
        panic!("fails to check UFT-8 validity of file stem: {stem:#?}")
    };

    let mut pathbuf = PathBuf::new();
    pathbuf.set_file_name(format!("llm_instrumented_{stem}.e"));
    pathbuf
}

fn name_subclass(name_base_class: &ClassName) -> ClassName {
    ClassName(format!("LLM_INSTRUMENTED_{name_base_class}"))
}

fn candidate_body_in_subclass(
    name_class: &ClassName,
    name_subclass: &ClassName,
    feature_to_fix: &Feature,
    body: String,
) -> String {
    EiffelSource::subclass_redefining_features(
        name_class,
        vec![(feature_to_fix, body)],
        name_subclass,
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::generators::constructor_api::CompletionParameters;
    use crate::lib::generators::constructor_api::MessageOut;
    use crate::lib::generators::constructor_api::LLM;
    use crate::lib::parser::Parser;

    #[ignore]
    #[tokio::test]
    async fn extract_from_code_output() -> anyhow::Result<()> {
        let llm = LLM::try_new().await?;
        let system_message_content = r#"You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code.            
"#;
        let user_message_content = r#"-- For the current class and its ancestors, the model is value: INTEGER
-- the model is implemented in Boogie.
-- For the argument other: NEW_INTEGER
--  the model is value: INTEGER
--    the model is terminal, no qualified call on it is allowed.
smaller (other: NEW_INTEGER): BOOLEAN
	do
		Result := value < other.value
	ensure
		Result = (value < other.value)
	end
	
"#;
        let messages = vec![
            MessageOut::new_system(system_message_content.to_string()),
            MessageOut::new_user(user_message_content.to_string()),
        ];
        let llm_parameters = CompletionParameters {
            messages,
            model: constructor_api::EnumLanguageModel::GeminiFlash,
            ..Default::default()
        };
        let output = llm.model_complete(&llm_parameters).await?;
        let specs: Vec<RoutineSpecification> = output
            .extract_multiline_code()
            .into_iter()
            .inspect(|code| eprintln!("{code}"))
            .map(|code| {
                let mut parser = Parser::new();
                parser
                    .feature_from_source(&code)
                    .expect("parsing must succed (possibly with error nodes).")
            })
            .map(|ft| ft.routine_specification())
            .inspect(|spec| eprintln!("{spec:#?}"))
            .collect();
        Ok(())
    }
}
