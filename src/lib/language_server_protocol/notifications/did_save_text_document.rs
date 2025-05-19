use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::notification::{DidSaveTextDocument, Notification};
use async_lsp::Result;
use std::ops::ControlFlow;
use std::path::PathBuf;
impl HandleNotification for DidSaveTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        tokio::spawn(async move {
            let mut ws = st.workspace.write().await;
            let pathbuf = PathBuf::from(params.text_document.uri.path());

            ws.reload(pathbuf).await;
        });

        ControlFlow::Continue(())
    }
}
