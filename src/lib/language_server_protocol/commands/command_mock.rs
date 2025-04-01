use super::Command;
use crate::lib::language_server_protocol::commands::Generators;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types;

#[derive(Default, Debug, Clone)]
pub struct MockCommand;

impl<'ws> From<(&'ws Workspace, Vec<serde_json::Value>)> for MockCommand {
    fn from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Self {
        assert!(value.0.is_mock());
        assert!(value.1.is_empty());
        Self::default()
    }
}
impl Command<'_> for MockCommand {
    const NAME: &'static str = "mock";

    const TITLE: &'static str = "Mock";

    fn arguments(&self) -> Vec<serde_json::Value> {
        Vec::new()
    }

    async fn generate_edits(
        &self,
        _generators: &Generators,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        todo!()
    }
}
