use crate::lib::language_server_protocol::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::{request, DocumentSymbolParams, DocumentSymbolResponse};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;

impl HandleRequest for request::DocumentSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: DocumentSymbolParams,
    ) -> impl Future<Output = Result<Self::Result, ResponseError>> + Send + 'static {
        async move {
            let path = params.text_document.uri.path().as_ref();
            let workspace = st.workspace.read().await;

            Ok(workspace.class(path).map(|class| {
                let symbol = class.to_document_symbol().unwrap_or_else(|e| {
                    unreachable!(
                        "fails to convert class {:#?} ot document symbol with error {:#?}",
                        class.name(),
                        e
                    )
                });
                DocumentSymbolResponse::Nested(vec![symbol])
            }))
        }
    }
}
