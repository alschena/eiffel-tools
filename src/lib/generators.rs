use crate::lib::code_entities::prelude::*;
use crate::lib::parser::Parser;
use crate::lib::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use contract::RoutineSpecification;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

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
                reply
                    .extract_multiline_code()
                    .into_iter()
                    .filter_map(|candidate| {
                        info!("candidate:\t{candidate}");
                        let mut parser = Parser::new();
                        parser.feature_from_source(&candidate).map_or_else(
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

    /// Returns maybe the fixed body the routine.
    pub async fn fix_routine(
        &self,
        path: &Path,
        feature: &Feature,
        error_message: String,
    ) -> Result<Option<String>> {
        let prompt = prompt::Prompt::feature_fixes(feature, path, error_message)
            .await?
            .to_messages();

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
                .filter_map(|maybe_response| {
                    maybe_response.as_ref()
                        .inspect_err(|e| {
                            warn!(
                                target = "llm",
                                "An LLM processing the feature fix has returned the error: {:#?}",
                                e
                            )
                        })
                        .ok()
                })
                .flat_map(|response| response.extract_multiline_code().into_iter())
                .inspect(|candidate| {
                    info!(target: "llm", "candidate:\t{candidate}");
                })
                .filter_map(|candidate| {
                    let mut parser = Parser::new();

                    let ft = parser
                        .feature_from_source(&candidate)
                        .inspect_err(|e| {
                            info!(target: "llm", "fails to parse LLM generated feature with error: {e:#?}");
                        })
                        .ok()?;

                    ft.body_source_unchecked(candidate).inspect_err(|e| info!(target: "llm", "fails to extract body of candidate feature with error: {:#?}", e)).ok()
                })
                .inspect(|filtered_candidate| {
                    info!(target: "llm", "candidate of correct body:\t{:#?}", filtered_candidate);
                })
                .next();

        if completion_response_processed.is_none() {
            info!(target:"llm", "llm proposes no candidate.");
        }

        Ok(completion_response_processed)
    }
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
