use crate::lib::processed_file::ProcessedFile;
use crate::lib::transformer::Generator;
use crate::lib::workspace::Workspace;
use async_lsp::ClientSocket;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Clone)]
pub struct ServerState {
    pub client: ClientSocket,
    pub workspace: Arc<RwLock<Workspace>>,
    pub generators: Arc<RwLock<Vec<Generator>>>,
}
impl ServerState {
    pub fn new(client: ClientSocket) -> ServerState {
        let generator = Arc::new(RwLock::new(Vec::new()));
        let binding: Arc<RwLock<Vec<Generator>>> = generator.clone();
        tokio::spawn(async {
            let mut generator = binding.write_owned().await;
            for _ in 0..10 {
                Generator::try_new().await.map_or_else(
                    |e| warn!("fail to create generator with error:\t{e:#?}"),
                    |new_generator| generator.push(new_generator),
                )
            }
        });

        ServerState {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
            generators: generator,
        }
    }
    pub async fn find_file(&self, path: &Path) -> Option<ProcessedFile> {
        let ws = self.workspace.read().await;
        ws.find_file(path).map(|f| f.to_owned())
    }
}
