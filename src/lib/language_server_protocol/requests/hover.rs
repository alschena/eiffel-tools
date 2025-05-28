use super::super::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::request;
use async_lsp::ResponseError;
use async_lsp::Result;
impl HandleRequest for request::HoverRequest {
    async fn handle_request(
        _st: ServerState,
        _params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        Ok(None)
    }
}
