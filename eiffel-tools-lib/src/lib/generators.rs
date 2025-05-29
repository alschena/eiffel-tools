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
    llms: Vec<Arc<constructor_api::Llm>>,
}

impl Generators {
    pub async fn add_new(&mut self) {
        let Ok(llm) = constructor_api::Llm::try_new().await else {
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
            messages: prompt.into_llm_chat_messages(),
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
            .into_llm_chat_messages();

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
impl Generators {
    pub fn mock() -> Self {
        Generators { llms: Vec::new() }
    }
}
