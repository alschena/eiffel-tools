use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::ClientSocket;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::anyhow;

#[derive(Clone)]
pub struct ServerState {
    pub(super) client: ClientSocket,
    pub(super) workspace: Arc<RwLock<Workspace>>,
}
impl ServerState {
    pub fn new(client: ClientSocket) -> ServerState {
        ServerState {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
        }
    }
    pub async fn find_file(&self, path: &Path) -> Option<ProcessedFile> {
        let ws = self.workspace.read().expect("workspace must be readable.");
        ws.find_file(path).map(|f| f.to_owned())
    }
}
