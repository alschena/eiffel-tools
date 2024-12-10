use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::{request, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
use std::path;

impl HandleRequest for request::DocumentSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: DocumentSymbolParams,
    ) -> impl Future<Output = Result<Self::Result, ResponseError>> + Send + 'static {
        async move {
            let path: path::PathBuf = params.text_document.uri.path().into();
            let read_workspace = st.workspace.read().await;
            let file = read_workspace
                .files()
                .into_iter()
                .find(|&x| x.path() == path);
            if let Some(file) = file {
                let class = file.class();
                let symbol: DocumentSymbol = (class)
                    .try_into()
                    .expect("class conversion to document symbol");
                let classes = vec![symbol];
                return Ok(Some(DocumentSymbolResponse::Nested(classes)));
            } else {
                return Ok(None);
            }
        }
    }
}
