use super::common::{HandleNotification, HandleRequest, ServerState};
use async_lsp::lsp_types::{
    notification, request, HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
};
use async_lsp::{ResponseError, Result};
use std::future::Future;
use std::ops::ControlFlow;
impl HandleRequest for request::Initialize {
    fn handle_request(
        _st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            eprintln!("Initialize with {params:?}");
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
                    document_symbol_provider: Some(OneOf::Left(true)),
                    workspace_symbol_provider: Some(OneOf::Left(true)),
                    ..ServerCapabilities::default()
                },
                server_info: None,
            })
        }
    }
}
impl HandleNotification for notification::Initialized {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}
