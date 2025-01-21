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

            ws.find_file_mut(path).map(|file| {
                let mut parser = tree_sitter::Parser::new();
                parser
                    .set_language(&tree_sitter_eiffel::LANGUAGE.into())
                    .expect("Error loading Eiffel grammar");
                file.reload(&mut parser)
            });
        });

        ControlFlow::Continue(())
    }
}
