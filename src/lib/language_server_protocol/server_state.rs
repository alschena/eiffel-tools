use crate::lib::config::System;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use async_lsp::ClientSocket;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinSet;

#[derive(Clone)]
pub struct ServerState {
    pub client: ClientSocket,
    pub workspace: Arc<RwLock<Workspace>>,
    pub tasks: Arc<Mutex<Vec<Task>>>,
}
pub enum Task {
    LoadConfig(System),
}
impl From<System> for Task {
    fn from(value: System) -> Self {
        Task::LoadConfig(value)
    }
}
impl ServerState {
    pub fn new(client: ClientSocket) -> ServerState {
        ServerState {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub async fn find_file(&self, path: &Path) -> Option<ProcessedFile> {
        let ws = self.workspace.read().await;
        ws.find_file(path).map(|f| f.to_owned())
    }
    pub async fn process_task(&mut self) {
        let tasks = self.tasks.clone();
        let mut tasks = tasks.lock().await;
        let Some(task) = tasks.pop() else { return };
        match task {
            Task::LoadConfig(system) => {
                let eiffel_files = system.eiffel_files();
                let mut set = JoinSet::new();
                eiffel_files.into_iter().for_each(|filepath| {
                    let mut parser = tree_sitter::Parser::new();
                    parser
                        .set_language(&tree_sitter_eiffel::LANGUAGE.into())
                        .expect("Error loading Eiffel grammar");
                    set.spawn(
                        async move { ProcessedFile::new(&mut parser, filepath.to_owned()).await },
                    );
                });
                let files = (set.join_all().await)
                    .into_iter()
                    .filter_map(|file| file)
                    .collect();
                let ws = self.workspace.clone();
                let mut ws = ws.write().await;
                ws.set_files(files);
            }
        }
    }
    pub async fn add_task(&mut self, task: Task) {
        let tasks = self.tasks.clone();
        let mut tasks = tasks.lock().await;
        tasks.push(task)
    }
}
