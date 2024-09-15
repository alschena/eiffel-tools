use crate::lib::code_entities::*;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::{notification, request, SymbolKind, Url};
use async_lsp::router;
use async_lsp::ClientSocket;
use async_lsp::Result;
use async_lsp::{lsp_types, ResponseError};
use std::future::Future;
use std::ops::ControlFlow;
use std::sync::{Arc, RwLock};
use tracing::info;
impl TryFrom<&Class<'_>> for lsp_types::Location {
    type Error = ();

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let range = value.range().clone().into();
        let uri = value
            .location()
            .expect("Valid location of class")
            .try_into()
            .expect("Extraction of location from class");
        Ok(Self { uri, range })
    }
}
impl TryFrom<&Class<'_>> for lsp_types::SymbolInformation {
    type Error = ();
    fn try_from(value: &Class<'_>) -> std::result::Result<Self, Self::Error> {
        let name = value.name().into();
        let kind = SymbolKind::CLASS;
        let tags = None;
        let deprecated = None;
        let container_name = None;
        match value.try_into() {
            Err(e) => Err(e),
            Ok(location) => Ok(Self {
                name,
                kind,
                tags,
                deprecated,
                location,
                container_name,
            }),
        }
    }
}
impl TryFrom<&Location> for Url {
    type Error = ();

    fn try_from(value: &Location) -> std::result::Result<Self, ()> {
        Self::from_file_path(value.path.clone())
    }
}
impl From<Point> for async_lsp::lsp_types::Position {
    fn from(value: Point) -> Self {
        Self {
            line: value.row.try_into().expect("Failed to convert row"),
            character: value.column.try_into().expect("Failed to convert column"),
        }
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
impl From<Range> for async_lsp::lsp_types::Range {
    fn from(value: Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}
#[derive(Clone)]
pub struct ServerState {
    pub(super) client: ClientSocket,
    pub(super) workspace: Arc<RwLock<Vec<ProcessedFile>>>,
    pub(super) counter: i32,
}
pub struct TickEvent;
pub trait HandleRequest: request::Request {
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
impl HandleRequest for request::GotoDefinition {
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
            workspace: Arc::new(RwLock::new(Vec::new())),
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
