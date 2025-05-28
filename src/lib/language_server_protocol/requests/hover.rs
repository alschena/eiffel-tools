use super::super::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::request;
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
impl HandleRequest for request::HoverRequest {
    fn handle_request(
        _st: ServerState,
        _params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move { Ok(None) }
    }
}
