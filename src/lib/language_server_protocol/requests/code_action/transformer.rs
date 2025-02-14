use super::utils::{text_edit_add_postcondition, text_edit_add_precondition};
use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use anyhow::{anyhow, Context};
use async_lsp::lsp_types::{CodeActionDisabled, Url, WorkspaceEdit};
use async_lsp::Result;
use contract::{Block, Fix, Postcondition, Precondition, RoutineSpecification};
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use tracing::info;

mod prompt;

#[cfg(feature = "gemini")]
use gemini::{self, ToResponseSchema};

#[cfg(feature = "gemini")]
#[derive(Default)]
pub struct LLM {
    model_config: gemini::Config,
    request_config: gemini::GenerationConfig,
    client: reqwest::Client,
}

#[cfg(feature = "gemini")]
impl LLM {
    pub fn new() -> Self {
        let mut request_config =
            gemini::GenerationConfig::from(RoutineSpecification::to_response_schema());
        request_config.set_temperature(Some(2.0));
        Self {
            request_config,
            ..Default::default()
        }
    }
    fn model_config(&self) -> &gemini::Config {
        &self.model_config
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    pub async fn add_contracts_to_feature(
        &self,
        feature: &Feature,
        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, CodeActionDisabled> {
        let system_classes = workspace.system_classes().collect::<Vec<_>>();
        let mut prompt = prompt::Prompt::default();
        prompt.set_feature_src_with_contract_holes(feature, file)?;
        prompt.set_full_model_text(feature, file.class(), &system_classes);

        let mut request = gemini::Request::from(prompt.text());
        request.set_config(self.request_config.clone());

        let Ok(response) = request
            .process_with_async_client(self.model_config(), self.client())
            .await
        else {
            return Err(CodeActionDisabled {
                reason: "fails to process llm request".to_string(),
            });
        };

        info!(target:"gemini", "Request to llm: {request:?}\nResponse from llm: {response:?}");

        let system_classes = workspace.system_classes().collect::<Vec<_>>();
        let responses = response.parsed().inspect(|s: &RoutineSpecification| {
            info!(target: "gemini", "Generated routine specifications\n\tpreconditions:\t{}\n\tpostconditions:\t{}", s.precondition, s.postcondition);
        });
        let mut corrected_responses = responses
            .filter_map(|mut spec: RoutineSpecification| {
                if spec.fix(&system_classes, file.class(), feature) {
                    Some(spec)
                } else {None}
            })
            .inspect(|s: &RoutineSpecification| {
                info!(target: "gemini", "Fixed routine specificatins\n\tpreconditions:\t{}\n\tpostcondition{}", s.precondition, s.postcondition);
            });
        let Some(spec) = corrected_responses.next() else {
            return Err(CodeActionDisabled {
                reason: "No added specification for routine was produced".to_string(),
            });
        };
        let url = Url::from_file_path(file.path()).expect("convert file path to url.");

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
}

#[cfg(feature = "ollama")]
#[derive(Default)]
pub struct LLM {
    model: ollama_rs::Ollama,
    prompt: prompt::Prompt,
}

impl LLM {
    pub fn new() -> Result<LLM, CodeActionDisabled> {
        let host = String::from("https://constructor.app/platform/code/vz-eu3/730cfa9dbebe42229e418c5291c6cb93/proxy/11434/");

        let mut headers = HeaderMap::new();
        headers.insert(
            "Cookie",
            format!(
                "ap_access_token={}",
                std::env::var("CONSTRUCTOR_AP_ACCESS_TOKEN").map_err(|_| CodeActionDisabled {
                    reason: String::from(
                        "CONSTRUCTOR_AP_ACCESS_TOKEN must be a variable in the environment",
                    ),
                })?
            )
            .parse()
            .map_err(|_| CodeActionDisabled {
                reason: String::from("Token related invalid header value."),
            })?,
        );

        let model = ollama_rs::Ollama::new_with_request_headers(host, 11434, headers);
        Ok(Self {
            model,
            ..Default::default()
        })
    }
    pub async fn add_contracts_to_feature(
        &self,
        feature: &Feature,

        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, CodeActionDisabled> {
        let system_classes = workspace.system_classes().collect::<Vec<_>>();
        let prompt = prompt::Prompt::for_feature_specification(
            feature,
            &file.class().full_extended_model(&system_classes),
            file,
            &system_classes,
        )?;
        todo!()
    }
}
