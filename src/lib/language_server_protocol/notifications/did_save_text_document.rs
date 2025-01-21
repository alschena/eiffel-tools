use crate::lib::language_server_protocol::prelude::*;
use crate::lib::language_server_protocol::server_state::Task;
use async_lsp::lsp_types::notification::{DidSaveTextDocument, Notification};
use async_lsp::Result;
use std::ops::ControlFlow;
use std::path::PathBuf;
impl HandleNotification for DidSaveTextDocument {
    fn handle_notification(
        mut st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        let path = PathBuf::from(params.text_document.uri.path());
        let _ = tokio::spawn(async move {
            st.add_task(Task::ReloadFile(path)).await;
        });
        ControlFlow::Continue(())
    }
}
