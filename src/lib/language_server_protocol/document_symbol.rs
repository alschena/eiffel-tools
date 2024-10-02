use super::common::{HandleRequest, ServerState};
use crate::lib::code_entities::class::Class;
use crate::lib::code_entities::feature::Feature;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::{
    request, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
use std::path;
impl TryFrom<DocumentSymbol> for Class {
    type Error = anyhow::Error;

    fn try_from(value: DocumentSymbol) -> std::result::Result<Self, Self::Error> {
        let name = value.name;
        let kind = value.kind;
        let range = value.range.try_into()?;
        debug_assert_eq!(kind, SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v
                .into_iter()
                .map(|x| Feature::try_from(x).expect("Document symbol to feature"))
                .collect(),
            None => Vec::new(),
        };
        Ok(Class::from_name_range(name, range))
    }
}
impl TryFrom<DocumentSymbol> for Feature {
    type Error = anyhow::Error;

    fn try_from(value: DocumentSymbol) -> std::result::Result<Self, Self::Error> {
        let name = value.name;
        let kind = value.kind;
        let range = value.range.try_into()?;
        debug_assert_ne!(kind, SymbolKind::CLASS);
        Ok(Feature::from_name_and_range(name, range))
    }
}
impl TryFrom<&Feature> for DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Feature) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let range = value.range().clone().try_into()?;
        Ok(DocumentSymbol {
            name,
            detail: None,
            kind: SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        })
    }
}
impl TryFrom<&Class> for DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let features = value.features();
        let range = value.range().clone().try_into()?;
        let children: Option<Vec<DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| {
                    x.as_ref()
                        .try_into()
                        .expect("feature conversion to document symbol")
                })
                .collect(),
        );
        Ok(DocumentSymbol {
            name,
            detail: None,
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children,
        })
    }
}
impl HandleRequest for request::DocumentSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: DocumentSymbolParams,
    ) -> impl Future<Output = Result<Self::Result, ResponseError>> + Send + 'static {
        async move {
            let path: path::PathBuf = params.text_document.uri.path().into();
            // Read borrow
            {
                let read_workspace = st.workspace.read().unwrap();
                let file = read_workspace.iter().find(|&x| x.path == path);
                if let Some(file) = file {
                    let class: Class = file.try_into().expect("Parse class");
                    let symbol: DocumentSymbol = (&class)
                        .try_into()
                        .expect("class conversion to document symbol");
                    let classes = vec![symbol];
                    return Ok(Some(DocumentSymbolResponse::Nested(classes)));
                }
            }
            // Write borrow
            {
                let mut write_workspace = st.workspace.write().unwrap();
                debug_assert!(write_workspace.iter().find(|&x| x.path == path).is_none());

                let mut parser = tree_sitter::Parser::new();
                parser
                    .set_language(tree_sitter_eiffel::language())
                    .expect("Error loading Eiffel grammar");
                let file = ProcessedFile::new(&mut parser, path);
                let class: Class = (&file).try_into().expect("Parse class");
                let symbol: DocumentSymbol = (&class)
                    .try_into()
                    .expect("Class conversion to document symbol");
                let document_symbols = vec![symbol];

                write_workspace.push(file);
                return Ok(Some(DocumentSymbolResponse::Nested(document_symbols)));
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
