use crate::lib::code_entities::prelude::*;
use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::{request, CodeActionOrCommand};
use async_lsp::ResponseError;
use async_lsp::Result;

mod generate_routine_specification;

impl HandleRequest for request::CodeActionRequest {
    async fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let ws = st.workspace.read().await;
        let mut generator = st.generator.write().await;
        let path = params
            .text_document
            .uri
            .to_file_path()
            .expect("Uri must convert to file path.");
        let point: Point = params
            .range
            .end
            .try_into()
            .expect("LSP-range must convert to internal `Point`");

        let file = ws.find_file(&path);
        let system_classes = ws.system_classes().collect::<Vec<_>>();

        let generate_routine_specification = generate_routine_specification::code_action(
            generator.as_mut(),
            file,
            &system_classes,
            &point,
        )
        .await;

        Ok(Some(vec![CodeActionOrCommand::CodeAction(
            generate_routine_specification,
        )]))
    }
}

#[cfg(test)]
mod test {}
