use super::Command;
use crate::lib::code_entities::prelude::Class;
use crate::lib::code_entities::prelude::Feature;
use crate::lib::code_entities::prelude::Range;
use crate::lib::code_entities::Indent;
use crate::lib::language_server_protocol::commands::lsp_types;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaikonInstrumenter<'ws> {
    workspace: &'ws Workspace,
    file: &'ws ProcessedFile,
    class: &'ws Class,
    feature: &'ws Feature,
}

impl<'ws> DaikonInstrumenter<'ws> {
    pub fn try_new(workspace: &'ws Workspace, filepath: &Path, feature_name: &str) -> Result<Self> {
        let file = workspace
            .find_file(filepath)
            .with_context(|| format!("Fails to find file of path: {filepath:#?}"))?;
        let class = file.class();
        let feature = class
            .features()
            .iter()
            .find(|&ft| ft.name() == feature_name)
            .with_context(|| {
                format!("Fails to find in file: {file:#?} feature of name: {feature_name}")
            })?;
        Ok(Self {
            class,
            workspace,
            file,
            feature,
        })
    }

    pub fn instrument_body_start_and_end(&self) -> Result<[lsp_types::TextEdit; 2]> {
        let system_classes = self.workspace.system_classes();
        let class_fields: Vec<_> = self
            .class
            .immediate_and_inherited_features(&system_classes)
            .into_iter()
            .filter_map(|ft| {
                (ft.parameters().is_empty() && ft.return_type().is_some()).then_some(ft)
            })
            .collect();
        let parameters = self.feature.parameters();

        let Some(Range { start, end }) = self.feature.body_range() else {
            bail!(
                "fails find the range of the body of the feature to instrument: {:#?}",
                &self.feature
            )
        };

        let indentation_string =
            (0..=Feature::INDENTATION_LEVEL + 1).fold(String::new(), |acc, _| format!("{acc}\t"));

        let print_class_fields_instructions = class_fields.iter().fold(String::new(), |acc, ft| {
            format!(
                r#"{acc}
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line"#,
                ft.name()
            )
        });

        let print_class_fields_and_parameters_instructions =
            parameters
                .names()
                .iter()
                .fold(print_class_fields_instructions, |acc, param_name| {
                    format!(
                        r#"{acc}
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line"#,
                        param_name
                    )
                });

        let collapsed_start_range = Range::new_collapsed(*start);
        let text_edit_start = lsp_types::TextEdit {
            range: collapsed_start_range.try_into()?,
            new_text: print_class_fields_and_parameters_instructions.clone(),
        };

        let collapsed_end_range = Range::new_collapsed(*end);
        let text_edit_end = lsp_types::TextEdit {
            range: collapsed_end_range.try_into()?,
            new_text: print_class_fields_and_parameters_instructions,
        };

        Ok([text_edit_start, text_edit_end])
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for DaikonInstrumenter<'ws> {
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

impl<'ws> Command<'ws> for DaikonInstrumenter<'ws> {
    const NAME: &'static str = "instrument_feature_for_daikon";

    const TITLE: &'static str = "Instrument feature for Daikon";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let path = self.file.path();
        let Ok(serialized_filepath) = serde_json::to_value(path) else {
            unreachable!("fails to serialize path: {path:#?}")
        };
        let feature = self.feature;
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn generate_edits(
        &self,
        _generators: &crate::lib::generators::Generators,
    ) -> Result<lsp_types::WorkspaceEdit> {
        let url = lsp_types::Url::from_file_path(self.file.path()).map_err(|_| {
            anyhow!("if on unix path must be absolute. if on windows path must have disk prefix")
        })?;

        Ok(lsp_types::WorkspaceEdit::new(HashMap::from([(
            url,
            self.instrument_body_start_and_end()?.into(),
        )])))
    }
}

#[cfg(test)]
mod tests {}
