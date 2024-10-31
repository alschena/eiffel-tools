use super::prelude::*;
use async_lsp::lsp_types::request::Request;
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;

mod code_action;
mod document_symbol;
mod goto_definition;
mod hover;
mod initialize;
mod workspace_document_symbol;

pub trait HandleRequest: Request {
    fn handle_request(
        st: ServerState,
        params: <Self as Request>::Params,
    ) -> impl Future<Output = Result<<Self as Request>::Result, ResponseError>> + Send + 'static;
}
