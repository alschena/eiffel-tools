use crate::lib::code_entities::prelude::*;
use crate::lib::language_server_protocol::commands;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types;
use async_lsp::lsp_types::request;
use async_lsp::lsp_types::CodeActionOrCommand;
use async_lsp::ResponseError;
use async_lsp::Result;
use std::path::PathBuf;

impl HandleRequest for request::CodeActionRequest {
    async fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let ws = st.workspace.read().await;
        let params = CodeActionParams(params);
        let path = params.path_owned();
        let point: Point = params.cursor();

        let add_specification_to_routine_under_cursor_command =
            commands::Commands::try_new_add_routine_specification_at_cursor(&ws, &path, point)
                .map_err(|e| {
                    ResponseError::new(
                        async_lsp::ErrorCode::INTERNAL_ERROR,
                        format!(
                    "fails to create command to generate routine specifications with error: {e}"
                ),
                    )
                })?
                .command();
        let daikon_instrumentation_to_routine_under_cursor_command = commands::Commands::try_new_instrument_routine_at_cursor_for_daikon(&ws, &path, point)
                .map_err(|e| {
                    ResponseError::new(
                        async_lsp::ErrorCode::INTERNAL_ERROR,
                        format!(
                    "fails to create command to instrument routine at cursor: {:#?} for daikon with error: {}", point,e
                ),
                    )
                })?
                .command();

        Ok(Some(vec![
            CodeActionOrCommand::Command(add_specification_to_routine_under_cursor_command),
            CodeActionOrCommand::Command(daikon_instrumentation_to_routine_under_cursor_command),
        ]))
    }
}

struct CodeActionParams(lsp_types::CodeActionParams);
impl From<lsp_types::CodeActionParams> for CodeActionParams {
    fn from(value: lsp_types::CodeActionParams) -> Self {
        Self(value)
    }
}

impl CodeActionParams {
    fn path_owned(&self) -> PathBuf {
        self.0
            .text_document
            .uri
            .to_file_path()
            .expect("Uri must convert to file path.")
    }
    fn cursor(&self) -> Point {
        self.0
            .range
            .end
            .try_into()
            .expect("LSP-range must convert to internal `Point`")
    }
}

#[cfg(test)]
mod test {}
