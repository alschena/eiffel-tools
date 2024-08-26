use crate::lib::code_entities::*;
use async_lsp::lsp_types::{
    notification, request, DocumentSymbol, Hover, HoverContents, HoverProviderCapability,
    InitializeResult, MarkedString, MessageType, OneOf, ServerCapabilities, ShowMessageParams,
    SymbolKind,
};
use async_lsp::router;
use async_lsp::ClientSocket;
use async_lsp::{lsp_types, ResponseError};
use async_lsp::{Error, Result};
use std::future::Future;
use std::ops::ControlFlow;
use std::time::Duration;
use tracing::{info, Level};

impl From<DocumentSymbol> for Feature<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_ne!(kind, SymbolKind::CLASS);
        Feature::from_name_and_range(name, range.into())
    }
}

impl From<DocumentSymbol> for Class<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_eq!(kind, SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v.into_iter().map(|x| x.into()).collect(),
            None => Vec::new(),
        };
        Class::from_name_range(name, range.into())
    }
}

impl From<async_lsp::lsp_types::Position> for Point {
    fn from(value: async_lsp::lsp_types::Position) -> Self {
        Self {
            row: value
                .line
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
            column: value
                .character
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
        }
    }
}

impl From<async_lsp::lsp_types::Range> for Range {
    fn from(value: async_lsp::lsp_types::Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

#[derive(Clone)]
pub struct ServerState {
    client: ClientSocket,
    counter: i32,
}

pub struct TickEvent;
pub trait HandleRequest: lsp_types::request::Request {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static;
}
pub trait HandleNotification: lsp_types::notification::Notification {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()>;
}

impl HandleRequest for request::Initialize {
    fn handle_request(
        _st: ServerState,
        params: <Self as lsp_types::request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as lsp_types::request::Request>::Result, ResponseError>>
           + Send
           + 'static {
        async move {
            eprintln!("Initialize with {params:?}");
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
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

impl HandleRequest for request::HoverRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        let client = st.client.clone();
        let counter = st.counter;
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            client
                .notify::<notification::ShowMessage>(ShowMessageParams {
                    typ: MessageType::INFO,
                    message: "Hello LSP".into(),
                })
                .unwrap();
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "I am a hover text {counter}!"
                ))),
                range: None,
            }))
        }
    }
}

impl HandleRequest for request::GotoDefinition {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move { unimplemented!() }
    }
}

impl HandleRequest for request::DocumentSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move { unimplemented!() }
    }
}

impl HandleNotification for notification::DidChangeConfiguration {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidOpenTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidChangeTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidCloseTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

pub struct Router<T>(router::Router<T>);

impl<T> From<router::Router<T>> for Router<T> {
    fn from(value: router::Router<T>) -> Self {
        Self(value)
    }
}

impl<T> From<Router<T>> for router::Router<T> {
    fn from(value: Router<T>) -> Self {
        value.0
    }
}

impl Router<ServerState> {
    pub fn new(client: &ClientSocket) -> Router<ServerState> {
        let kernel = router::Router::new(ServerState {
            client: client.clone(),
            counter: 0,
        });
        Router(kernel)
    }
    pub fn set_handler_request<T: HandleRequest + 'static>(&mut self) {
        self.0
            .request::<T, _>(|st, params| T::handle_request(st.clone(), params));
    }
    pub fn set_handler_notification<T: HandleNotification + 'static>(&mut self) {
        self.0
            .notification::<T>(|st, params| T::handle_notification(st.clone(), params));
    }
    pub fn set_tick_event(&mut self) {
        self.0.event::<TickEvent>(|st, _| {
            info!("tick");
            st.counter += 1;
            ControlFlow::Continue(())
        });
    }
}
