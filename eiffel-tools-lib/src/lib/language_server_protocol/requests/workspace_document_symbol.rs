use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::{request, WorkspaceSymbolResponse};
use async_lsp::ResponseError;
use async_lsp::Result;
use tracing::warn;
impl HandleRequest for request::WorkspaceSymbolRequest {
    async fn handle_request(
        st: ServerState,
        _params: <Self as request::Request>::Params,
    ) -> Result<<Self as request::Request>::Result, ResponseError> {
        let workspace = st.workspace.read().await;

        let symbols = workspace
            .system_classes()
            .iter()
            .map(|class| (class, workspace.path(class.name())))
            .flat_map(|(class, path)| {
                class
                    .to_symbol_information(path)
                    .inspect_err(|e| {
                        warn!(
                            "fails to create symbol information for class {:#?} with error {:#?}",
                            class.name(),
                            e
                        )
                    })
                    .ok()
            })
            .collect();
        Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
    }
}
