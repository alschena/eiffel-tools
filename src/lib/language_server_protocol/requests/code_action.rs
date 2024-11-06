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

        let (edit, disabled) = match file {
            Some(file) => {
                let point = params
                    .range
                    .end
                    .try_into()
                    .expect("fails to convert lsp-point to eiffel point");
                let model = transformer::LLM::default();
                match model.add_contracts_at_point(point, &file).await {
                    (None, None) => {
                        (None, Some(CodeActionDisabled{reason: String::from("fails for internal errors with the llm processing. For more information read the log file")}))
                    },
                    a @ _ => a,
                }
            }
            None => (
                None,
                Some(CodeActionDisabled {
                    reason: "The current file has not been parsed yet.".to_string(),
                }),
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
