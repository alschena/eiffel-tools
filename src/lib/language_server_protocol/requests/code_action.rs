use crate::lib::code_entities::prelude::*;
use crate::lib::code_entities::ValidSyntax;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::{request, CodeAction, CodeActionDisabled, CodeActionOrCommand};
use async_lsp::lsp_types::{Url, WorkspaceEdit};
use async_lsp::ResponseError;
use async_lsp::Result;
use contract::RoutineSpecification;
use gemini;
use gemini::ToResponseSchema;
use std::collections::HashMap;
use std::fmt::Display;
use tracing::{info, warn};
mod transformer;
mod utils;
use utils::*;

#[derive(Debug)]
enum Error<'a> {
    CodeActionDisabled(&'a str),
    PassThroughError(&'a str),
}
impl<'a> Error<'a> {
    fn resolve(&self) -> Option<CodeActionDisabled> {
        match self {
            Self::CodeActionDisabled(reason) => Some(CodeActionDisabled {
                reason: reason.to_string(),
            }),
            Self::PassThroughError(reason) => {
                warn!("{reason}");
                None
            }
        }
    }
}
impl<'a> Display for Error<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CodeActionDisabled(s) => write!(f, "{s}"),
            Error::PassThroughError(s) => write!(
                f,
                "fails with {s}, but the process can continue. Look at log file for more information."
            ),
        }
    }
}

impl HandleRequest for request::CodeActionRequest {
    async fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let ws = st.workspace.read().await;
        let path = params
            .text_document
            .uri
            .to_file_path()
            .expect("fails to convert uri of code action parameter in usable path.");
        let file = ws.find_file(&path);

        let (edit, disabled) = match file {
            Some(file) => {
                let mut model = transformer::LLM::default();
                model.set_file(&file);
                model.set_workspace(&ws);
                let point: Point = params
                    .range
                    .end
                    .try_into()
                    .expect("fails to convert lsp-point to eiffel point");
                match model.add_contracts_at_point(&point, &ws).await {
                    Ok(edit) => (Some(edit), None),
                    Err(e) => (
                        None,
                        Some(e.resolve().expect(
                            "all these failures must be resolved disabling the code action.",
                        )),
                    ),
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
