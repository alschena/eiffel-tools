use crate::code_entities::prelude::*;
use crate::generators::Generators;
use crate::workspace::Workspace;
use anyhow::Context;
use async_lsp::lsp_types;
use serde_json;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ClassSpecificationGenerator<'ws> {
    workspace: &'ws Workspace,
    path: &'ws Path,
}

impl ClassSpecificationGenerator<'_> {
    fn class(&self) -> &Class {
        self.workspace.class(self.path).unwrap_or_else(|| panic!("fails to find a class in the workspace for creating a class specification generator at path: {:#?} ",self.path))
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for ClassSpecificationGenerator<'ws> {
    type Error = anyhow::Error;

    fn try_from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Result<Self, Self::Error> {
        let ws = value.0;
        let mut args = value.1;

        let classname = args.pop().with_context(|| {
                "The construction of the command to generate class specifications requires the name of a class."
            })?;

        let classname: String = serde_json::from_value(classname)?;

        let path = ws.path(&ClassName(classname));

        ClassSpecificationGenerator::try_new(ws, path)
    }
}

impl<'ws> super::Command<'ws> for ClassSpecificationGenerator<'ws> {
    const TITLE: &'static str = "Add specifications to class";
    const NAME: &'static str = "add_specifications_to_class";

    fn arguments(&self) -> Vec<serde_json::Value> {
        match serde_json::to_value(self.path) {
            Ok(serialized_filepath) => vec![serialized_filepath],
            Err(_) => unreachable!("path: {:#?} must be serialized.", self.path),
        }
    }

    async fn generate_edits(
        &self,
        _generators: &Generators,
    ) -> anyhow::Result<Option<lsp_types::WorkspaceEdit>> {
        let _ = self.class();
        todo!()
    }
}

impl<'ws> ClassSpecificationGenerator<'ws> {
    pub fn try_new(workspace: &'ws Workspace, path: &'ws Path) -> anyhow::Result<Self> {
        Ok(Self { workspace, path })
    }
    pub fn new(workspace: &'ws Workspace, path: &'ws Path) -> Self {
        Self { workspace, path }
    }
}
