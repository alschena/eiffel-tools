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
            let path = params
                .text_document
                .uri
                .to_file_path()
                .expect("Path of target document of code action");
            let processed_file = workspace
                .files()
                .iter()
                .find(|&x| x.path == path)
                .expect("Code action on not yet parsed file");
            let range = params.range.try_into().expect("Range conversion");
            let surrounding_feature = processed_file.feature_around(range);

            let title = "Add contracts to current routine".into();
            let kind = None;
            let diagnostics = None;
            let command = None;
            let is_preferred = Some(false);
            let data = None;
            let mut response = CodeActionResponse::new();
            match surrounding_feature {
                Some(feature) => {
                    let model = transformer::LLM::default();
                    let (pre, post) = model.add_contracts(&feature);
                    let range = feature
                        .range_end_preconditions()
                        .clone()
                        .try_into()
                        .expect("Convert range to lsp-type range");
                    let edit = lsp_types::WorkspaceEdit::new(HashMap::from([
                        (
                            params.text_document.uri.clone(),
                            vec![lsp_types::TextEdit {
                                range,
                                new_text: format!("{pre}"),
                            }],
                        ),
                        (
                            params.text_document.uri,
                            vec![lsp_types::TextEdit {
                                range,
                                new_text: format!("{post}"),
                            }],
                        ),
                    ]));
                    let disabled = None;
                    let code_action = CodeAction {
                        title,
                        kind,
                        diagnostics,
                        edit: Some(edit),
                        command,
                        is_preferred,
                        disabled,
                        data,
                    };
                    // let command = Command::new(title, "add_contracts_routine".to_string(), None);
                    response.push(CodeActionOrCommand::CodeAction(code_action));
                    // response.push(CodeActionOrCommand::Command(command));
                    Ok(Some(response))
                }
                None => {
                    let disabled = Some(CodeActionDisabled {
                        reason: "The cursor is not surrounded by a feature".to_string(),
                    });
                    let edit = None;
                    let code_action = CodeAction {
                        title,
                        kind,
                        diagnostics,
                        edit,
                        command,
                        is_preferred,
                        disabled,
                        data,
                    };
                    response.push(CodeActionOrCommand::CodeAction(code_action));
                    // response.push(CodeActionOrCommand::Command(command));
                    Ok(Some(response))
                }
            }
        }
    }
}

#[cfg(test)]
mod test {}
