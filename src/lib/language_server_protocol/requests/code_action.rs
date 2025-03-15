use crate::lib::code_entities::prelude::*;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types;
use async_lsp::lsp_types::request;
use async_lsp::lsp_types::CodeActionOrCommand;
use async_lsp::ResponseError;
use async_lsp::Result;
use std::path::PathBuf;

mod generate_routine_specification;
use generate_routine_specification::SourceGenerationContext;

impl HandleRequest for request::CodeActionRequest {
    async fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let ws = st.workspace.read().await;
        let generators = st.generators.write().await;
        let params = CodeActionParams(params);
        let path = params.path_owned();
        let point: Point = params.cursor();

        let file = ws.find_file(&path).ok_or_else(|| {
            ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, "fail to find file.")
        })?;

        let generate_routine_specification = SourceGenerationContext::new(&ws, file, point)
            .code_action(&generators)
            .await;

        Ok(Some(vec![CodeActionOrCommand::CodeAction(
            generate_routine_specification,
        )]))
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
