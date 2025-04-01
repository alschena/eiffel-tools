use crate::lib::code_entities::prelude::*;
use crate::lib::generators::Generators;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use anyhow::Context;
use async_lsp::lsp_types;
use serde_json;
use std::path;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ClassSpecificationGenerator<'ws> {
    workspace: &'ws Workspace,
    file: &'ws ProcessedFile,
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
        let file = ws
            .files()
            .iter()
            .find(|&file| {
                let ClassName(name) = file.class().name();
                name == &classname
            })
            .with_context(|| {
                "The name ``{classname}'' does not match the name of a class in the workspace"
            })?;

        return Ok(ClassSpecificationGenerator::try_new(ws, file.path())?);
    }
}

impl<'ws> super::Command<'ws> for ClassSpecificationGenerator<'ws> {
    const TITLE: &'static str = "Add specifications to class";
    const NAME: &'static str = "add_specifications_to_class";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let filepath = self.path();
        match serde_json::to_value(filepath) {
            Ok(serialized_filepath) => vec![serialized_filepath],
            Err(_) => unreachable!("filepath: {filepath:#?} must be serialized."),
        }
    }

    async fn generate_edits(
        &self,
        generators: &Generators,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        todo!()
    }
}

impl<'ws> ClassSpecificationGenerator<'ws> {
    pub fn try_new(workspace: &'ws Workspace, filepath: &Path) -> anyhow::Result<Self> {
        let file = workspace
            .find_file(filepath)
            .with_context(|| "Fails to find file of path: {filepath} in workspace")?;
        Ok(Self { workspace, file })
    }
    pub fn new(workspace: &'ws Workspace, file: &'ws ProcessedFile) -> Self {
        Self { workspace, file }
    }
    fn path(&self) -> &path::Path {
        self.file.path()
    }
    fn class(&self) -> &Class {
        &self.file.class()
    }
    fn features(&self) -> &[Feature] {
        &self.class().features()
    }
}
