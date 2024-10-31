use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::{
    request::{Initialize, Request},
    HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
};
use async_lsp::ResponseError;
use std::future::Future;

impl HandleRequest for Initialize {
    fn handle_request(
        _st: ServerState,
        params: <Self as Request>::Params,
    ) -> impl Future<Output = Result<<Self as Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
                    document_symbol_provider: Some(OneOf::Left(true)),
                    workspace_symbol_provider: Some(OneOf::Left(true)),
                    code_action_provider: Some(true.into()),
                    ..ServerCapabilities::default()
                },
                server_info: None,
            })
        }
    }
}
