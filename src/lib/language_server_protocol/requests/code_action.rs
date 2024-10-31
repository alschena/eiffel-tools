use crate::lib::code_entities::prelude::*;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::{self, request, CodeAction, CodeActionDisabled, CodeActionOrCommand};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::collections::HashMap;
mod transformer;

impl HandleRequest for request::CodeActionRequest {
    async fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let path = params
            .text_document
            .uri
            .to_file_path()
            .expect("fails to convert uri of code action parameter in usable path.");
        let file = st.find_file(&path).await;

        let (disabled, edit) = match file {
            Some(file) => match file.feature_around_point(
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
                        let feature_src = file.feature_src(&feature).expect("file should contain feature");
                        let (pre, post) = model.add_contracts_async(feature_src).await.expect("llm fails to produce contracts");
                            (None, Some(lsp_types::WorkspaceEdit::new(HashMap::from([ (params.text_document.uri, vec![
                                    lsp_types::TextEdit {
                                        range: postcondition_range_end.clone().try_into().expect("range should convert to lsp-type range."),
                                        new_text: if feature.is_postcondition_block_present() {
                                            format!("{post}")} else {format!(
                                                    "{}",
                                                    contract::Block::<contract::Postcondition> {
                                                        item: Some(post),
                                                        range: postcondition_range_end,
                                                        keyword: contract::Keyword::Ensure,
                                                    }
                                                )}
                                    },
                                    lsp_types::TextEdit {
                                        range: precondition_range_end.clone().try_into().expect("range should convert to lsp-type range."),
                                        new_text: if feature.is_precondition_block_present() {
                                            format!("{pre}")} else {format!(
                                                    "{}",
                                                    contract::Block::<contract::Precondition> {
                                                        item: Some(pre),
                                                        range: precondition_range_end,
                                                        keyword: contract::Keyword::Require,
                                                    }
                                                )}
                                    },
                            ])
                            ]))))
                        }
                        (None, None) => (Some(CodeActionDisabled {reason: String::from("The surrounding feature does not support adding pre or post conditions.")}), None),
                        _ => unreachable!()
                    }
                }
                None => (
                    Some(CodeActionDisabled {
                        reason: "The cursor is not surrounded by a feature".to_string(),
                    }),
                    None,
                ),
            },
            None => (
                Some(CodeActionDisabled {
                    reason: "The current file has not been parsed yet.".to_string(),
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

#[cfg(test)]
mod test {}
