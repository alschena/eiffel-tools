use super::common::{HandleRequest, ServerState};
use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use anyhow::Context;
use async_lsp::lsp_types::{
    self, request, CodeAction, CodeActionDisabled, CodeActionOrCommand, WorkspaceEdit,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::collections::HashMap;
use std::future::Future;
mod transformer;

impl HandleRequest for request::CodeActionRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let workspace = st.workspace.read().unwrap();

            let path = params
                .text_document
                .uri
                .to_file_path()
                .expect("fails to convert uri of code action parameter in usable path.");

            let (disabled, edit) = match workspace
                .find_file(&path)
                .expect("fails calling code action on not yet parsed file.")
                .feature_around_point(
                    params
                        .range
                        .end
                        .try_into()
                        .expect("fails to convert lsp position to eiffel point."),
                ) {
                Some(feature) => {
                    match (feature.range_end_preconditions(), feature.range_end_postconditions()) {
                        (Some(precondition_range_end), Some(postcondition_range_end)) => {
                        let model = transformer::LLM::default();
                        let src = std::fs::read(params.text_document.uri.path()).expect("fails to read from file.");
                        let (pre, post) = model.add_contracts(String::from_utf8(src).expect("fails to convert byte vector to string").as_ref()).expect("llm fails to produce contracts");
                            (None, Some(lsp_types::WorkspaceEdit::new(HashMap::from([ (params.text_document.uri, vec![
                                    lsp_types::TextEdit {
                                        range: postcondition_range_end.clone().try_into().expect("fails to convert range to lsp-type range."),
                                        new_text: match feature.is_postcondition_block_present() {
                                            true =>  format!("{post}"),
                                            false => {format!(
                                                    "{}",
                                                    contract::Block::<contract::Postcondition> {
                                                        item: Some(post),
                                                        range: postcondition_range_end,
                                                        keyword: contract::Keyword::Ensure,
                                                    }
                                                )}
                                        }
                                    },
                                    lsp_types::TextEdit {
                                        range: precondition_range_end.clone().try_into().expect("fails to convert range to lsp-type range."),
                                        new_text: match feature.is_precondition_block_present() {
                                            true => format!("{pre}"),
                                            false => {
                                                format!(
                                                    "{}",
                                                    contract::Block::<contract::Precondition> {
                                                        item: Some(pre),
                                                        range: precondition_range_end,
                                                        keyword: contract::Keyword::Require,
                                                    }
                                                )
                                            }
                                        },
                                    },
                            ])
                            ]))))
                        }
                        (None, None) => (Some(CodeActionDisabled {reason: String::from("The surrounding feature does not support the addition of pre and post conditions.")}), None),
                        _ => unreachable!()
                    }
                }
                None => (
                    Some(CodeActionDisabled {
                        reason: "The cursor is not surrounded by a feature".to_string(),
                    }),
                    None,
                ),
            };

            Ok(Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
                title: String::from("Add contracts to current routine"),
                kind: None,
                diagnostics: None,
                edit,
                command: None,
                is_preferred: Some(false),
                disabled,
                data: None,
            })]))
        }
    }
}

#[cfg(test)]
mod test {}
