use super::utils::{text_edit_add_postcondition, text_edit_add_precondition};
use super::Error;
use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::{Url, WorkspaceEdit};
use async_lsp::Result;
use contract::{Block, Fix, Postcondition, Precondition, RoutineSpecification};
use gemini;
use gemini::ToResponseSchema;
use std::collections::HashMap;
use tracing::info;

mod prompt;

#[derive(Default)]
pub struct LLM {
    model_config: gemini::Config,
    client: reqwest::Client,
}

impl LLM {
    fn model_config(&self) -> &gemini::Config {
        &self.model_config
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
impl LLM {
    async fn add_contracts_to_feature(
        &self,
        feature: &Feature,
        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, Error<'static>> {
        let system_classes = workspace.system_classes().collect::<Vec<_>>();
        let mut prompt = prompt::Prompt::default();
        prompt.append_preamble_text();
        prompt.append_feature_src_with_contract_holes(feature, file)?;
        prompt.append_full_model_text(feature, file.class(), &system_classes);

        let mut request = gemini::Request::from(prompt.into_string());

        let mut request_config =
            gemini::GenerationConfig::from(RoutineSpecification::to_response_schema());
        request_config.set_temperature(Some(2.0));
        request.set_config(request_config);

        let Ok(response) = request
            .process_with_async_client(self.model_config(), self.client())
            .await
        else {
            return Err(Error::CodeActionDisabled("fails to process llm request"));
        };

        info!(target:"gemini", "Request to llm: {request:?}\nResponse from llm: {response:?}");

        let system_classes = workspace.system_classes().collect::<Vec<_>>();
        let responses = response.parsed().inspect(|s: &RoutineSpecification| {
            info!(target: "gemini", "Generated routine specifications\n\tpreconditions:\t{}\n\tpostconditions:\t{}", s.precondition, s.postcondition);
        });
        let mut fixed_responses = responses
            .filter_map(|mut spec: RoutineSpecification| {
                if spec.fix(&system_classes, file.class(), feature) {
                    Some(spec)
                } else {None}
            })
            .inspect(|s: &RoutineSpecification| {
                info!(target: "gemini", "Fixed routine specificatins\n\tpreconditions:\t{}\n\tpostcondition{}", s.precondition, s.postcondition);
            });
        let Some(spec) = fixed_responses.next() else {
            return Err(Error::CodeActionDisabled(
                "No added specification for routine was produced",
            ));
        };
        let Ok(url) = Url::from_file_path(file.path()) else {
            return Err(Error::PassThroughError("convert file path to url."));
        };

        Ok(WorkspaceEdit::new(HashMap::from([(
            url,
            vec![
                text_edit_add_precondition(
                    &feature,
                    feature.point_end_preconditions().unwrap().clone(),
                    spec.precondition,
                ),
                text_edit_add_postcondition(
                    &feature,
                    feature.point_end_postconditions().unwrap().clone(),
                    spec.postcondition,
                ),
            ],
        )])))
    }
    pub async fn add_contracts_at_point(
        &self,
        point: &Point,
        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, Error<'static>> {
        let Some(feature) = file.feature_around_point(point) else {
            return Err(Error::CodeActionDisabled(
                "A valid feature must surround the cursor.",
            ));
        };
        Ok(self
            .add_contracts_to_feature(feature, file, workspace)
            .await?)
    }
}
