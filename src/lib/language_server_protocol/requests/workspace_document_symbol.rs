use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::{request, SymbolInformation, WorkspaceSymbolResponse};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
impl HandleRequest for request::WorkspaceSymbolRequest {
    fn handle_request(
        st: ServerState,
        _params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let read_workspace = st.workspace.read().await;
            let files = read_workspace.files();
            let symbol_information: Vec<SymbolInformation> = files
                .iter()
                .map(|x| {
                    x.try_into()
                        .expect("Class convertable to symbol information")
                })
                .collect();
            Ok(Some(WorkspaceSymbolResponse::Flat(symbol_information)))
        }
    }
}
