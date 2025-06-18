use crate::code_entities::prelude::*;
use crate::eiffel_source::EiffelSource;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::autoproof;
use crate::generators::Generators;
use crate::language_server_protocol::commands::fix_routine::path::PathBuf;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_lsp::lsp_types;
use serde_json;
use std::path;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone)]
pub struct FixRoutine<'ws> {
    workspace: &'ws Workspace,
    path: PathBuf,
    feature: &'ws Feature,
    fixed_routine_body: Option<String>,
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
            fixed_routine_body: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn feature(&self) -> &Feature {
        &self.feature
    }

    pub fn fixed_routine_body(&self) -> Option<&String> {
        self.fixed_routine_body.as_ref()
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for FixRoutine<'ws> {
    type Error = anyhow::Error;

    fn try_from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Result<Self, Self::Error> {
        let workspace = value.0;
        let mut arguments = value.1;
        let feature_name = arguments.pop().with_context(
            || "Fails to retrieve the second argument (feature name) to add routine specification.",
        )?;
        let feature_name: String = serde_json::from_value(feature_name)?;
        let filepath = arguments.pop().with_context(
            || "Fails to retrieve the first argument (file path) to add routine specification.",
        )?;
        let filepath: PathBuf = serde_json::from_value(filepath)?;
        Self::try_new(workspace, &filepath, &feature_name)
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
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name().as_ref()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn generate_edits(
        &self,
        _generators: &Generators,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        let Range { mut start, end } = self
            .feature
            .body_range()
            .with_context(|| {
                format!(
                    "fails to get body range from feature {:#?}",
                    self.feature.name()
                )
            })?
            .to_owned();

        start.shift_right(2); // Compensate for `do` keyword.

        let body_range = Range { start, end }.try_into()?;

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

        Ok(self.fixed_routine_body.clone().map(workspace_edit))
    }

    async fn side_effect(&mut self, generators: &Generators) -> anyhow::Result<()> {
        {
            let workspace = self.workspace;
            let path = &self.path;
            let feature = self.feature;

            let class = workspace
                .class(path)
                .ok_or_else(|| anyhow!("fails to find loaded class at path: {:#?}", path))?;

            let feature_body = feature.body_source_unchecked_at_path(path).await?;

            write_to_feature_redefinition(path, class.name(), feature, feature_body).await?;

            let mut number_of_tries = 0;
            let mut feature_verified: Option<String> = None;
            while let VerificationResult::Failure(error_message) =
                autoproof(&name_subclass(class.name()), Some(feature.name()))
                    .await
                    .with_context(|| "fails to await for autoproof.")?
            {
                if number_of_tries <= 5 {
                    number_of_tries += 1;
                } else {
                    break;
                }

                let maybe_candidate = generators
                    .fix_body(workspace, path, feature.name(), error_message)
                    .await?;

                // Write text edits to disk.
                if let Some(candidate) = maybe_candidate {
                    feature_verified = Some(candidate.clone());

                    info!(target: "llm", "Writing candidate to subclass file:\n{}", candidate);
                    write_to_feature_redefinition(path, class.name(), feature, candidate).await?;
                }
            }

            if let VerificationResult::Success =
                autoproof(&name_subclass(class.name()), Some(feature.name()))
                    .await
                    .with_context(|| "fails to await autoproof result.")?
            {
                info!(
                target: "autoproof",
                "The feature's body generated by the LLM verifies. Moving the body into the initial feature.");
                self.fixed_routine_body = feature_verified;
            }

            Ok(())
        }
    }
}

async fn write_to_feature_redefinition(
    path: &Path,
    class_name: &ClassName,
    feature: &Feature,
    feature_body: String,
) -> Result<()> {
    // Write subclass redefining `feature` copy-pasting the body
    tokio::fs::write(
        path_llm_feature_redefinition(path),
        candidate_body_in_subclass(
            class_name,
            &name_subclass(class_name),
            feature,
            feature_body,
        ),
    )
    .await
    .map_err(|e| {
        anyhow!(
            "fails to write feature redefinition for LLMs fix to file with error: {:#?}",
            e
        )
    })
}

fn path_llm_feature_redefinition(path: &Path) -> PathBuf {
    let Some(stem) = path.file_stem() else {
        panic!("fails to get file stem (filename without extension) of current file.")
    };

    let Some(stem) = stem.to_str() else {
        panic!("fails to check UFT-8 validity of file stem: {stem:#?}")
    };

    let mut pathbuf = PathBuf::new();
    pathbuf.set_file_name(format!("llm_instrumented_{stem}.e"));
    pathbuf
}

fn name_subclass(name_base_class: &ClassName) -> ClassName {
    ClassName(format!("LLM_INSTRUMENTED_{name_base_class}"))
}

fn candidate_body_in_subclass(
    name_class: &ClassName,
    name_subclass: &ClassName,
    feature_to_fix: &Feature,
    body: String,
) -> String {
    EiffelSource::subclass_redefining_features(
        name_class,
        vec![(feature_to_fix, body)],
        name_subclass,
    )
    .to_string()
}
