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

pub struct LLMBuilder<'a, 'b> {
    model_config: gemini::Config,
    client: reqwest::Client,
    file: Option<&'a ProcessedFile>,
    workspace: Option<&'b Workspace>,
}
impl<'a, 'b> LLMBuilder<'a, 'b> {
    pub fn set_file(mut self, file: &'a ProcessedFile) -> LLMBuilder<'a, 'b> {
        self.file = Some(file);
        self
    }
    pub fn set_workspace(mut self, ws: &'b Workspace) -> LLMBuilder<'a, 'b> {
        self.workspace = Some(ws);
        self
    }
    pub fn build(self) -> LLM<'a, 'b> {
        LLM {
            model_config: self.model_config,
            client: self.client,
            file: self.file.expect("llmbuilder has a file set."),
            workspace: self.workspace.expect("llmbuilder has a workspace set."),
        }
    }
}

impl Default for LLMBuilder<'_, '_> {
    fn default() -> Self {
        Self {
            model_config: gemini::Config::default(),
            client: reqwest::Client::new(),
            file: None,
            workspace: None,
        }
    }
}

pub struct LLM<'a, 'b> {
    model_config: gemini::Config,
    client: reqwest::Client,
    file: &'a ProcessedFile,
    workspace: &'b Workspace,
}
impl<'a, 'b> LLM<'a, 'b> {
    fn model_config(&self) -> &gemini::Config {
        &self.model_config
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    fn target_url(&self) -> Result<Url, Error<'static>> {
        Url::from_file_path(self.file.path())
            .map_err(|_| Error::PassThroughError("fails to transform path into lsp_types::Url"))
    }
}
impl<'a, 'b> LLM<'a, 'b> {
    async fn request_specification(
        &self,
        feature: &Feature,
    ) -> Result<(RoutineSpecification, Point, Point), Error<'static>> {
        let file = self.file;
        let workspace = self.workspace;
        let Some(point_insert_preconditions) = feature.point_end_preconditions() else {
            return Err(Error::CodeActionDisabled(
                "Only attributes with an attribute block and routines support adding preconditions",
            ));
        };
        let Some(point_insert_postconditions) = feature.point_end_postconditions() else {
            return Err(Error::CodeActionDisabled("Only attributes with an attribute block and routines support adding postconditions"));
        };
        let precondition_hole = if feature.has_precondition() {
            format!(
                "\n{}<ADDED_PRECONDITION_CLAUSES>",
                Precondition::indentation_string()
            )
        } else {
            format!(
                "<NEW_PRECONDITION_BLOCK>\n{}",
                <Block<Precondition>>::indentation_string()
            )
        };
        let postcondition_hole = if feature.has_postcondition() {
            format!(
                "\n{}<ADDED_POSTCONDITION_CLAUSES>",
                Postcondition::indentation_string()
            )
        } else {
            format!(
                "<NEW_POSTCONDITION_BLOCK>\n{}",
                <Block<Postcondition>>::indentation_string()
            )
        };
        let injections = vec![
            (point_insert_preconditions, precondition_hole.as_str()),
            (point_insert_postconditions, postcondition_hole.as_str()),
        ];
        let Ok(feature_src) = file.feature_src_with_injections(&feature, injections.into_iter())
        else {
            return Err(Error::PassThroughError(
                "fails to extract source of feature from file",
            ));
        };
        let full_model_text;
        {
            let system_classes = workspace.system_classes().collect::<Vec<_>>();
            // TODO add models of arguments and Result.
            let local_models_setup = "The models of the current class and its ancestors are:\n";
            let mut text =
                file.class()
                    .full_model(&system_classes)
                    .fold(String::new(), |mut acc, model| {
                        acc.push_str(
                            format!("{}{model}", ClassModel::indentation_string()).as_str(),
                        );
                        acc.push('\n');
                        acc
                    });
            text.insert_str(0, local_models_setup);

            let parameters_text = feature.parameters().full_model(&system_classes).fold(
                String::new(),
                |mut acc, (name, models)| {
                    let parameter_model_setup = format!("The model of the arugment {name} is:\n");
                    let model_text = models.fold(String::new(), |mut acc, model| {
                        acc.push_str(
                            format!("{}{model}", ClassModel::indentation_string()).as_str(),
                        );
                        acc.push('\n');
                        acc
                    });
                    acc.push_str(&parameter_model_setup);
                    acc.push_str(&model_text);
                    acc
                },
            );

            text.push_str(&parameters_text);
            full_model_text = text;
        }
        let mut request = gemini::Request::from(format!(
            "You are an expert in formal methods, specifically design by contract for static verification. You are optionally adding model-based contracts to the following feature:```eiffel\n{feature_src}\n```\nRemember that model-based contract only refer to the model of the current class and the other classes referred by in the signature of the feature.\n{full_model_text}"
        ));

        let mut request_config =
            gemini::GenerationConfig::from(RoutineSpecification::to_response_schema());
        request_config.set_temperature(Some(2.0));
        request.set_config(request_config);

        match request
            .process_with_async_client(self.model_config(), self.client())
            .await
        {
            Ok(response) => {
                info!(target:"gemini", "Request to llm: {request:?}\nResponse from llm: {response:?}");

                let system_classes = workspace.system_classes().collect::<Vec<_>>();
                let responses = response.parsed().inspect(|s: &RoutineSpecification| {
                    info!(target: "gemini", "Generated routine specifications\n\tpreconditions:\t{}\n\tpostconditions:\t{}", s.precondition, s.postcondition);
                });
                let mut fixed_responses = responses
                    .filter_map(|mut spec: RoutineSpecification| {
                        spec.fix(&system_classes, file.class(), feature).ok()?;
                        if !spec.is_empty() {Some(spec)} else {None}
                    })
                    .inspect(|s: &RoutineSpecification| {
                        info!(target: "gemini", "Fixed routine specificatins\n\tpreconditions:\t{}\n\tpostcondition{}", s.precondition, s.postcondition);
                    });
                match fixed_responses.next() {
                    Some(spec) => Ok((
                        spec,
                        point_insert_preconditions.clone(),
                        point_insert_postconditions.clone(),
                    )),
                    None => Err(Error::CodeActionDisabled(
                        "No added specification for routine was produced",
                    )),
                }
            }
            Err(_) => Err(Error::CodeActionDisabled("fails to process llm request")),
        }
    }
    pub async fn add_contracts_at_point(
        &self,
        point: &Point,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, Error<'static>> {
        let file = self.file;
        let Some(feature) = file.feature_around_point(point) else {
            return Err(Error::CodeActionDisabled(
                "A valid feature must surround the cursor.",
            ));
        };
        let (
            RoutineSpecification {
                precondition: pre,
                postcondition: post,
            },
            precondition_insertion_point,
            postcondition_insertion_point,
        ) = self.request_specification(feature).await?;

        let url = self.target_url()?;
        Ok(WorkspaceEdit::new(HashMap::from([(
            url,
            vec![
                text_edit_add_precondition(&feature, precondition_insertion_point, pre),
                text_edit_add_postcondition(&feature, postcondition_insertion_point, post),
            ],
        )])))
    }
}
