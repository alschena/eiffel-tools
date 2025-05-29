use crate::language_server_protocol::prelude::*;
use async_lsp::Result;
use async_lsp::lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    Notification,
};
use std::ops::ControlFlow;
mod did_save_text_document;
mod initialization;

pub trait HandleNotification: Notification {
    fn handle_notification(
        st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<async_lsp::Result<()>, ()>;
}
impl HandleNotification for DidChangeConfiguration {
    fn handle_notification(
        _st: ServerState,
        _params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for DidOpenTextDocument {
    fn handle_notification(
        _st: ServerState,
        _params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for DidChangeTextDocument {
    fn handle_notification(
        _st: ServerState,
        _params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for DidCloseTextDocument {
    fn handle_notification(
        _st: ServerState,
        _params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}
