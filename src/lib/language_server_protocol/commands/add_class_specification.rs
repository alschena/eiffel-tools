use crate::lib::code_entities::prelude::*;
use crate::lib::generators::Generators;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::Context;
use async_lsp::lsp_types;
use serde_json;
use std::path;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ClassSpecificationGenerator<'ws> {
    workspace: &'ws Workspace,
    path: &'ws Path,
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

        return Ok(ClassSpecificationGenerator::try_new(ws, path)?);
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
        generators: &Generators,
    ) -> anyhow::Result<Option<lsp_types::WorkspaceEdit>> {
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
