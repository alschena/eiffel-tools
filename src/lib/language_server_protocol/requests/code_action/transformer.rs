use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use anyhow::{Context, Result};
use async_lsp::lsp_types::{CodeActionDisabled, TextEdit, Url, WorkspaceEdit};
use contract::{Postcondition, Precondition, RoutineSpecification};
use gemini;
use gemini::ToResponseSchema;
use std::collections::HashMap;
use tracing::{info, warn};

pub struct LLM(gemini::Config);
impl LLM {
    fn config(&self) -> &gemini::Config {
        &self.0
    }
}
impl Default for LLM {
    fn default() -> Self {
        Self(gemini::Config::default())
    }
}
impl LLM {
    pub async fn add_contracts_at_point(
        &self,
        point: Point,
        file: &ProcessedFile,
    ) -> (Option<WorkspaceEdit>, Option<CodeActionDisabled>) {
        let Some(feature) = file.feature_around_point(&point) else {
            return (
                None,
                Some(CodeActionDisabled {
                    reason: String::from("There is no feature surrounding the cursor"),
                }),
            );
        };
        let Ok(feature_src) = file.feature_src(&feature) else {
            warn!("fails to extract feature source from file");
            return (None, None);
        };

        let mut request_specification = gemini::Request::from(format!(
            "Add preconditions and postconditions to the following routine. DO NOT ADD CONTRACT CLAUSES ALREADY PRESENT.\n{}",
            feature_src
        ));
        request_specification.set_config(gemini::GenerationConfig::from(
            RoutineSpecification::to_response_schema(),
        ));
        let client = gemini::Request::new_async_client();
        let config = self.config();
        let Ok(response_specification) = request_specification
            .process_with_async_client(config.to_owned(), client)
            .await
        else {
            info!("fails to process llm request");
            return (None, None);
        };

        let mut specification = response_specification.parsed();

        let RoutineSpecification {
            precondition: pre,
            postcondition: post,
        } = specification
            .next()
            .expect("No specification for routine was produced");

        let Some(precondition_point_end) = feature.point_end_preconditions() else {
            return (
                None,
                Some(CodeActionDisabled {
                    reason: String::from("Only attributes with an attribute block and routines support adding preconditions"),
                }),
            );
        };
        let mut precondition_insert_point = precondition_point_end.clone();
        precondition_insert_point.reset_column();

        let Some(postcondition_point_end) = feature.point_end_postconditions() else {
            return (
                None,
                Some(CodeActionDisabled {
                    reason: String::from("Only attributes with an attribute block and routines support adding postconditions"),
                }),
            );
        };
        let mut postcondition_insert_point = postcondition_point_end.clone();
        postcondition_insert_point.reset_column();

        let Ok(url) = Url::from_file_path(file.path()) else {
            warn!("fails to transform path into lsp_types::Url");
            return (None, None);
        };
        let postcondition_text = if feature.has_postcondition() {
            format!("{post}")
        } else {
            format!(
                "{}",
                contract::Block::<contract::Postcondition> {
                    item: Some(post),
                    range: Range::new_collapsed(postcondition_insert_point),
                    keyword: contract::Keyword::Ensure,
                }
            )
        };
        let precondition_text = if feature.has_precondition() {
            format!("{pre}")
        } else {
            format!(
                "{}",
                contract::Block::<contract::Precondition> {
                    item: Some(pre),
                    range: Range::new_collapsed(precondition_insert_point),
                    keyword: contract::Keyword::Require,
                }
            )
        };
        (
            Some(WorkspaceEdit::new(HashMap::from([(
                url,
                vec![
                    TextEdit {
                        range: Range::new_collapsed(postcondition_point_end.clone())
                            .try_into()
                            .expect("range should convert to lsp-type range."),
                        new_text: postcondition_text,
                    },
                    TextEdit {
                        range: Range::new_collapsed(precondition_point_end.clone())
                            .try_into()
                            .expect("range should convert to lsp-type range."),
                        new_text: precondition_text,
                    },
                ],
            )]))),
            None,
        )
    }
}
