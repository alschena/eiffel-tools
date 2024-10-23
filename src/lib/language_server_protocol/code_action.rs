use super::common::{HandleRequest, ServerState};
use crate::lib::code_entities::prelude::*;
use async_lsp::lsp_types::{
    self, request, CodeAction, CodeActionDisabled, CodeActionOrCommand, CodeActionResponse,
    Command, SymbolInformation,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
mod transformer;

impl HandleRequest for request::CodeActionRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let workspace = st.workspace.read().unwrap();

            let (disabled, edit) = match workspace
                .files()
                .iter()
                .find(|&x| {
                    x.path
                        == params
                            .text_document
                            .uri
                            .to_file_path()
                            .expect("fails to convert uri of code action parameter in usable path.")
                })
                .expect("fails calling code action on not yet parsed file.")
                .feature_around(
                    params
                        .range
                        .try_into()
                        .expect("fails to convert lsp-range to internal range."),
                ) {
                Some(feature) => {
                    match feature.range_end_preconditions() {
                        Some(range) => {
                        let model = transformer::LLM::default();
                        let (pre, post) = model.add_contracts(&feature);
                            let range_lsp = range
                                .clone()
                                .try_into()
                                .expect("fails to convert range to lsp-type range.");
                            (None, Some(lsp_types::WorkspaceEdit::new(HashMap::from([
                                (
                                    params.text_document.uri.clone(),
                                    vec![lsp_types::TextEdit {
                                        range: range_lsp,
                                        new_text: match feature.is_precondition_block_present() {
                                            true => format!("{pre}"),
                                            false => {
                                                format!(
                                                    "{}",
                                                    ContractBlock::<Precondition> {
                                                        item: Some(pre),
                                                        range: range,
                                                        keyword: ContractKeyword::Require,
                                                    }
                                                )
                                            }
                                        },
                                    }],
                                ),
                                // (
                                //     params.text_document.uri,
                                //     vec![lsp_types::TextEdit {
                                //         range,
                                //         new_text: format!("{post}"),
                                //     }],
                                // ),
                            ]))))
                        }
                        None => (Some(CodeActionDisabled {reason: String::from("The surrounding feature does not support the addition of preconditions.")}), None),
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
