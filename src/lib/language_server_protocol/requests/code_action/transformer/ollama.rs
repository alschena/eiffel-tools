use super::prompt;
use super::text_edit_add_postcondition;
use super::text_edit_add_precondition;
use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::{CodeActionDisabled, Url, WorkspaceEdit};
use async_lsp::Result;
use contract::{Fix, RoutineSpecification};
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::parameters::FormatType;
use ollama_rs::generation::parameters::JsonStructure;
use reqwest::header::HeaderMap;
use std::collections::HashMap;

#[cfg(feature = "ollama")]
#[derive(Default)]
pub struct LLM {
    model: ollama_rs::Ollama,
    prompt: prompt::Prompt,
}

#[cfg(feature = "ollama")]
impl LLM {
    pub fn new() -> Result<LLM, CodeActionDisabled> {
        let host = String::from("https://constructor.app/platform/code/vz-eu3/730cfa9dbebe42229e418c5291c6cb93/proxy/11434/");

        let mut headers = HeaderMap::new();
        headers.insert(
            "Cookie",
            format!(
                "ap_access_token={}; ap_refresh_token={}",
                std::env::var("CONSTRUCTOR_AP_ACCESS_TOKEN").map_err(|_| CodeActionDisabled {
                    reason: String::from(
                        "CONSTRUCTOR_AP_ACCESS_TOKEN must be a variable in the environment",
                    ),
                })?,
                std::env::var("CONSTRUCTOR_AP_REFRESH_TOKEN").map_err(|_| CodeActionDisabled {
                    reason: String::from(
                        "CONSTRUCTOR_AP_REFRESH_TOKEN must be a variable in the environment",
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
        let current_class = file.class();
        let url = Url::from_file_path(file.path()).expect("convert file path to url.");

        let prompt = prompt::Prompt::for_feature_specification(
            feature,
            &current_class
                .name()
                .model_extended(&system_classes)
                .unwrap_or_default(),
            file,
            &system_classes,
        )?;
        let request = LLM::generation_request(&prompt);
        let generation_response =
            self.model
                .generate(request)
                .await
                .map_err(|e| CodeActionDisabled {
                    reason: format!("{e}"),
                })?;
        let response = generation_response.response;
        let mut routine_specification: RoutineSpecification =
            response.parse().map_err(|e| CodeActionDisabled {
                reason: format!("parse error {e:?}"),
            })?;

        // Fix routine specification.
        let corrected_responses = routine_specification
            .fix(&system_classes, current_class, feature)
            .then_some(routine_specification);

        let spec = corrected_responses.ok_or_else(|| CodeActionDisabled {
            reason: "No added specification for routine was produced".to_string(),
        })?;

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
    fn generation_request(prompt: &prompt::Prompt) -> GenerationRequest {
        let result = GenerationRequest::new(String::from("deepseek-6.7b"), prompt.text());
        result.format(FormatType::StructuredJson(JsonStructure::new::<
            RoutineSpecification,
        >()))
    }
}
