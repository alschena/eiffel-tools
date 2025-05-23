use crate::lib::code_entities::prelude::*;
use crate::lib::generators::Generators;
use crate::lib::language_server_protocol::commands::fix_routine::path::PathBuf;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use async_lsp::lsp_types;
use serde_json;
use std::path;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FixRoutine<'ws> {
    workspace: &'ws Workspace,
    path: PathBuf,
    feature: &'ws Feature,
}

impl<'ws> FixRoutine<'ws> {
    pub fn try_new(workspace: &'ws Workspace, filepath: &Path, feature_name: &str) -> Result<Self> {
        let class = workspace
            .class(filepath)
            .with_context(|| format!("fails to find loaded class at path: {:#?}", filepath))?;

        let feature = class
            .features()
            .iter()
            .find(|&ft| ft.name() == feature_name)
            .with_context(|| format!("Fails to find feature of name: {feature_name}"))?;

        Ok(Self {
            workspace,
            path: filepath.to_path_buf(),
            feature,
        })
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for FixRoutine<'ws> {
    type Error = anyhow::Error;

    fn try_from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Result<Self, Self::Error> {
        let workspace = value.0;
        let mut arguments = value.1;
        let feature_name = arguments.pop().with_context(|| {
            "Fails to retrieve the second argument (feature name) to add routine specification."
        })?;
        let feature_name: String = serde_json::from_value(feature_name)?;
        let filepath = arguments.pop().with_context(|| {
            "Fails to retrieve the first argument (file path) to add routine specification."
        })?;
        let filepath: PathBuf = serde_json::from_value(filepath)?;
        Self::try_new(workspace, &filepath, &feature_name)
    }
}

impl<'ws> FixRoutine<'ws> {
    fn system_classes(&self) -> &[Class] {
        self.workspace.system_classes()
    }
}

impl<'ws> super::Command<'ws> for FixRoutine<'ws> {
    const TITLE: &'static str = "Fix routine";
    const NAME: &'static str = "fix_routine";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let Ok(serialized_filepath) = serde_json::to_value(&self.path) else {
            unreachable!("fails to serialize path: {:#?}", self.path)
        };
        let feature = self.feature;
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn generate_edits(
        &self,
        generators: &Generators,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        let body_range = self
            .feature
            .body_range()
            .with_context(|| {
                format!(
                    "fails to get body range from feature {:#?}",
                    self.feature.name()
                )
            })?
            .to_owned()
            .try_into()?;

        let url = lsp_types::Url::from_file_path(self.path.clone())
            .map_err(|_| anyhow!("fails to convert file path to lsp url."))?;

        let workspace_edit = move |s| {
            lsp_types::WorkspaceEdit::new(
                [(
                    url,
                    vec![lsp_types::TextEdit {
                        range: body_range,
                        new_text: s,
                    }],
                )]
                .into(),
            )
        };

        Ok(generators
            .fix_routine(&self.workspace, &self.path, self.feature)
            .await?
            .map(|body_routine_verified| workspace_edit(body_routine_verified)))
    }
}
