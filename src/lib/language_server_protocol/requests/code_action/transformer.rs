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
    async fn add_specification_to_feature(
        &self,
        feature: &Feature,
        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<(RoutineSpecification, Point, Point), Error<'static>> {
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
        // TODO add model of Result.
        let full_model_text;
        {
            let system_classes = workspace.system_classes().collect::<Vec<_>>();

            let mut text = file
                .class()
                .full_extended_model(&system_classes)
                .fmt_indented(ClassModel::INDENTATION_LEVEL);

            if text.is_empty() {
                text.push_str("The current class and its ancestors have no model.");
            } else {
                text.insert_str(0, "Models of the current class and its ancestors:\n{}");
            }

            let parameters = feature.parameters();
            let parameters_models_fmt = parameters
                .types()
                .iter()
                .map(|t| {
                    t.class(system_classes.iter().copied())
                        .full_extended_model(&system_classes)
                })
                .map(|ext_model| ext_model.fmt_indented(ClassModel::INDENTATION_LEVEL));

            let parameters_models = parameters.names().iter().zip(parameters_models_fmt).fold(
                String::new(),
                |mut acc, (name, model_fmt)| {
                    acc.push_str("Model of the argument ");
                    acc.push_str(name);
                    acc.push(':');
                    acc.push('\n');
                    acc.push_str(model_fmt.as_str());
                    acc
                },
            );

            if !parameters_models.is_empty() {
                text.push_str(&parameters_models)
            }

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
                        if spec.fix(&system_classes, file.class(), feature) {
                            Some(spec)
                        } else {None}
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
        file: &ProcessedFile,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, Error<'static>> {
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
        ) = self
            .add_specification_to_feature(feature, file, workspace)
            .await?;

        let Ok(url) = Url::from_file_path(file.path()) else {
            return Err(Error::PassThroughError("convert file path to url."));
        };

        Ok(WorkspaceEdit::new(HashMap::from([(
            url,
            vec![
                text_edit_add_precondition(&feature, precondition_insertion_point, pre),
                text_edit_add_postcondition(&feature, postcondition_insertion_point, post),
            ],
        )])))
    }
}
