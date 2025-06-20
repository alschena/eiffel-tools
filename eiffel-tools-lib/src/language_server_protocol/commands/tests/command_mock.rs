use super::Command;
use crate::language_server_protocol::commands::Generators;
use crate::workspace::Workspace;
use anyhow::Result;
use async_lsp::lsp_types;

#[derive(Debug, Clone)]
pub struct MockCommand<'ws> {
    _workspace: &'ws Workspace,
}

impl<'ws> MockCommand<'ws> {
    pub fn new(workspace: &'ws Workspace) -> Self {
        Self {
            _workspace: workspace,
        }
    }
    pub fn test_function_with_arg(&self, s: String) -> String {
        s
    }
}

impl<'ws> From<(&'ws Workspace, Vec<serde_json::Value>)> for MockCommand<'ws> {
    fn from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Self {
        assert!(value.0.system_classes().is_empty());
        assert!(value.1.is_empty());
        Self {
            _workspace: value.0,
        }
    }
}
impl<'ws> Command<'ws> for MockCommand<'ws> {
    const NAME: &'static str = "mock";

    const TITLE: &'static str = "Mock";

    fn arguments(&self) -> Vec<serde_json::Value> {
        Vec::new()
    }

    async fn generate_edits(
        &self,
        _generators: &Generators,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        Ok(None)
    }
}
