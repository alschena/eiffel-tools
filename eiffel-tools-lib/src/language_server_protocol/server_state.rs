use crate::generators::Generators;
use crate::language_server_protocol::commands;
use crate::workspace::Workspace;
use async_lsp::ClientSocket;
use async_lsp::ResponseError;
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

            // `Generator.add_new()` will try to reuse the first knowledge model available.
            // If you want to add more generators, change the behavior of `LLMBuilder.build`
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
    pub async fn run(&self, mut command: commands::Commands<'_>) -> Result<(), ResponseError> {
        let client = &self.client;
        let generators = self.generators.read().await;
        command.run(client, &generators).await
    }
}
