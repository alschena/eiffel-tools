use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::notification::{DidSaveTextDocument, Notification};
use async_lsp::Result;
use std::ops::ControlFlow;
use std::path::Path;
impl HandleNotification for DidSaveTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        tokio::spawn(async move {
            let mut ws = st.workspace.write().await;
            let path = Path::new(params.text_document.uri.path());

            if let Some(file) = ws.find_file_mut(path) {
                file.reload().await
            };
        });

        ControlFlow::Continue(())
    }
}
