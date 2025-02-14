use crate::lib::code_entities::prelude::*;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::{
    request, CodeAction, CodeActionDisabled, CodeActionOrCommand, WorkspaceEdit,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::fmt::Display;
use tracing::warn;
mod transformer;
mod utils;

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
        let point: Point = params
            .range
            .end
            .try_into()
            .expect("fails to convert lsp-point to eiffel point");

        let file = ws.find_file(&path);

        let edit = match file {
            Some(file) => file_edits(file, &point, &ws).await,
            None => Err(CodeActionDisabled {
                reason: "The current file has not been parsed yet.".to_string(),
            }),
        };

        let (edit, disabled) = match edit {
            Ok(edit) => (Some(edit), None),
            Err(disabled) => (None, Some(disabled)),
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

async fn file_edits(
    file: &ProcessedFile,
    point: &Point,
    workspace: &Workspace,
) -> Result<WorkspaceEdit, CodeActionDisabled> {
    let model = transformer::LLM::new()?;
    let feature = Feature::feature_around_point(file.class().features().iter(), &point);
    match feature {
        Some(feature) => {
            model
                .add_contracts_to_feature(feature, file, workspace)
                .await
        }
        None => Err(CodeActionDisabled {
            reason: "Not in a valid feature.".to_string(),
        }),
    }
}

#[cfg(test)]
mod test {}
