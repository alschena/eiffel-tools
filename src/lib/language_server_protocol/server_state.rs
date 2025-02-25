use crate::lib::processed_file::ProcessedFile;
use crate::lib::transformer::Generator;
use crate::lib::workspace::Workspace;
use async_lsp::ClientSocket;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ServerState {
    pub client: ClientSocket,
    pub workspace: Arc<RwLock<Workspace>>,
    pub generator: Arc<RwLock<Option<Generator>>>,
}
impl ServerState {
    pub fn new(client: ClientSocket) -> ServerState {
        let generator = Arc::new(RwLock::new(None));
        let binding = generator.clone();
        tokio::spawn(async {
            let mut generator = binding.write_owned().await;
            *generator = Generator::try_new().await.ok();
        });

        ServerState {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
            generator,
        }
    }
    pub async fn find_file(&self, path: &Path) -> Option<ProcessedFile> {
        let ws = self.workspace.read().await;
        ws.find_file(path).map(|f| f.to_owned())
    }
}
