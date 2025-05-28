use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::request::{GotoDefinition, Request};
use async_lsp::ResponseError;
use std::future::Future;

impl HandleRequest for GotoDefinition {
    fn handle_request(
        _st: ServerState,
        _params: <Self as Request>::Params,
    ) -> impl Future<Output = Result<<Self as Request>::Result, ResponseError>> + Send + 'static
    {
        async move { unimplemented!() }
    }
}
