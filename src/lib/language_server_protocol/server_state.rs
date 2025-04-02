use crate::lib::generators::Generators;
use crate::lib::language_server_protocol::commands;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::ClientSocket;
use async_lsp::ResponseError;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ServerState {
    pub client: ClientSocket,
    pub workspace: Arc<RwLock<Workspace>>,
    pub generators: Arc<RwLock<Generators>>,
}
impl ServerState {
    pub fn new(client: ClientSocket) -> ServerState {
        let generators = Arc::new(RwLock::new(Generators::default()));
        let binding = generators.clone();
        tokio::spawn(async move {
            let mut generators = binding.write().await;
            for _ in 0..1 {
                generators.add_new().await
            }
        });

        ServerState {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
            generators,
        }
    }
    pub async fn find_file(&self, path: &Path) -> Option<ProcessedFile> {
        let ws = self.workspace.read().await;
        ws.find_file(path).map(|f| f.to_owned())
    }
    pub async fn run(&self, command: commands::Commands<'_>) -> Result<(), ResponseError> {
        let client = &self.client;
        let generators = self.generators.read().await;
        command.run(client, &generators).await
    }
}
