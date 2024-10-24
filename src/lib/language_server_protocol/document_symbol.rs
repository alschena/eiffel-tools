use super::common::{HandleRequest, ServerState};
use async_lsp::lsp_types::{
    request, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind,
};
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
            let read_workspace = st.workspace.read().unwrap();
            let file = read_workspace.files().iter().find(|&x| x.path == path);
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
#[cfg(test)]
mod test {
    use super::super::common::{Router, TickEvent};
    use super::*;
    use async_lsp::concurrency::ConcurrencyLayer;
    use async_lsp::panic::CatchUnwindLayer;
    use async_lsp::router;
    use async_lsp::server::LifecycleLayer;
    use async_lsp::tracing::TracingLayer;
    use async_lsp::{client_monitor::ClientProcessMonitorLayer, lsp_types::notification};
    use std::time::Duration;
    use tower::ServiceBuilder;
    #[tokio::test]
    async fn document_symbol() {
        let (server, _) = async_lsp::MainLoop::new_server(|client| {
            tokio::spawn({
                let client = client.clone();
                async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    loop {
                        interval.tick().await;
                        if client.emit(TickEvent).is_err() {
                            break;
                        }
                    }
                }
            });
            let mut router = Router::new(&client);
            router.set_handler_request::<request::Initialize>();
            router.set_handler_request::<request::HoverRequest>();
            router.set_handler_request::<request::GotoDefinition>();
            router.set_handler_request::<request::DocumentSymbolRequest>();
            router.set_handler_request::<request::WorkspaceSymbolRequest>();
            router.set_handler_notification::<notification::Initialized>();
            router.set_handler_notification::<notification::DidOpenTextDocument>();
            router.set_handler_notification::<notification::DidChangeTextDocument>();
            router.set_handler_notification::<notification::DidCloseTextDocument>();
            router.set_tick_event();

            ServiceBuilder::new()
                .layer(TracingLayer::default())
                .layer(LifecycleLayer::default())
                .layer(CatchUnwindLayer::default())
                .layer(ConcurrencyLayer::default())
                .layer(ClientProcessMonitorLayer::new(client))
                .service::<router::Router<_>>(router.into())
        });
        assert!(true);
    }
}
