use crate::language_server_protocol::prelude::*;
use async_lsp::ResponseError;
use async_lsp::lsp_types::request::{GotoDefinition, Request};

impl HandleRequest for GotoDefinition {
    async fn handle_request(
        _st: ServerState,
        _params: <Self as Request>::Params,
    ) -> Result<<Self as Request>::Result, ResponseError> {
        unimplemented!()
    }
}
