use super::prelude::*;
use async_lsp::lsp_types::notification;
use async_lsp::lsp_types::request;
use async_lsp::router;

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
    pub fn new(server_state: ServerState) -> Router<ServerState> {
        let kernel = router::Router::new(server_state);
        Router(kernel)
    }
    fn add_handler_request<T: HandleRequest + 'static>(&mut self) {
        self.0
            .request::<T, _>(|st, params| T::handle_request(st.clone(), params));
    }
    pub fn set_request_handlers(&mut self) {
        self.add_handler_request::<request::Initialize>();
        self.add_handler_request::<request::HoverRequest>();
        self.add_handler_request::<request::GotoDefinition>();
        self.add_handler_request::<request::DocumentSymbolRequest>();
        self.add_handler_request::<request::WorkspaceSymbolRequest>();
        self.add_handler_request::<request::CodeActionRequest>();
    }
    fn add_handler_notification<T: HandleNotification + 'static>(&mut self) {
        self.0
            .notification::<T>(|st, params| T::handle_notification(st.clone(), params));
    }
    pub fn set_notification_handlers(&mut self) {
        self.add_handler_notification::<notification::Initialized>();
        self.add_handler_notification::<notification::DidOpenTextDocument>();
        self.add_handler_notification::<notification::DidChangeTextDocument>();
        self.add_handler_notification::<notification::DidSaveTextDocument>();
        self.add_handler_notification::<notification::DidCloseTextDocument>();
        self.add_handler_notification::<notification::DidChangeConfiguration>();
    }
}
