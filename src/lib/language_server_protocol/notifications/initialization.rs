use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::notification::{Initialized, Notification};
use async_lsp::Result;
use std::ops::ControlFlow;
impl HandleNotification for Initialized {
    fn handle_notification(
        mut _st: ServerState,
        _params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}
