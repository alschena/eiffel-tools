use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::{notification, request};
use async_lsp::router;
use async_lsp::ClientSocket;
use async_lsp::Result;
use async_lsp::{lsp_types, ResponseError};
use std::future::Future;
use std::ops::ControlFlow;
use std::sync::{Arc, RwLock};
use tracing::info;
#[derive(Clone)]
pub struct ServerState {
    pub(super) client: ClientSocket,
    pub(super) workspace: Arc<RwLock<Workspace>>,
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
            workspace: Arc::new(RwLock::new(Workspace::new())),
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
