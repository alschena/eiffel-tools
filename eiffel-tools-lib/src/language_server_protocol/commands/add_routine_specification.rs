use crate::code_entities::prelude::*;
use crate::fix::FeaturePositionInSystem;
use crate::fix::Fix;
use crate::generators::Generators;
use crate::parser::Parser;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use async_lsp::lsp_types;
use contract::Postcondition;
use contract::Precondition;
use contract::RoutineSpecification;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone)]
pub struct RoutineSpecificationGenerator<'ws> {
    workspace: &'ws Workspace,
    path: &'ws Path,
    feature: &'ws Feature,
}

impl Deref for RoutineSpecificationGenerator<'_> {
    type Target = Workspace;

    fn deref(&self) -> &Self::Target {
        self.workspace
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for RoutineSpecificationGenerator<'ws> {
    type Error = anyhow::Error;

    fn try_from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Result<Self, Self::Error> {
        let (workspace, mut arguments) = value;

        let feature_name = arguments.pop().with_context(
            || "Fails to retrieve the second argument (feature name) to add routine specification.",
        )?;
        let feature_name: String = serde_json::from_value(feature_name)?;

        let filepath = arguments.pop().with_context(
            || "Fails to retrieve the first argument (file path) to add routine specification.",
        )?;
        let filepath: PathBuf = serde_json::from_value(filepath)?;

        let class = workspace
            .class(&filepath)
            .ok_or_else(|| anyhow!("fails to find class loaded from path {:#?}", filepath))?;

        let checked_path = workspace.path(class.name());

        Self::try_new(workspace, checked_path, &feature_name)
    }
}
impl<'ws> super::Command<'ws> for RoutineSpecificationGenerator<'ws> {
    const TITLE: &'static str = "Add specification to routine";
    const NAME: &'static str = "add_routine_specification";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let path = self.path;
        let Ok(serialized_filepath) = serde_json::to_value(path) else {
            unreachable!("fails to serialize path: {path:#?}")
        };
        let feature = self.feature;
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name().as_ref()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn generate_edits(
        &self,
        generators: &Generators,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        let more_routine_spec = generators
            .more_routine_specifications(self.feature, self.workspace, self.path)
            .await
            .inspect(|val| info!("Routine spec before fixes:\t{val:#?}"))
            .inspect_err(|e| info!("Error in routine spec before fixes:\t{e:#?}"))?;

        let fixed = self
            .fix_routine_specifications(more_routine_spec)
            .with_context(|| "fix routine specifications.")
            .inspect(|val| info!("Routine spec after fixes:\t{val:#?}"))
            .inspect_err(|e| info!("Error in routine spec after fixes:\t{e:#?}"))?;

        Ok(Some(
            self.routine_specification_edit(fixed)
                .inspect(|val| info!("Text edits of routine specs:\t{val:#?}"))
                .inspect_err(|e| info!("Error in text edit of routine specs:\t{e:#?}"))?,
        ))
    }
}

impl<'ws> RoutineSpecificationGenerator<'ws> {
    fn feature(workspace: &'ws Workspace, path: &Path, cursor: Point) -> Result<&'ws Feature> {
        workspace
            .feature_around(path, cursor)
            .with_context(|| "cursor is not around feature.")
    }

    pub fn try_new(workspace: &'ws Workspace, path: &'ws Path, feature_name: &str) -> Result<Self> {
        let class = workspace
            .class(path)
            .with_context(|| format!("Fails to find loaded class of path: {:#?}", path))?;

        let feature = class
            .features()
            .iter()
            .find(|&ft| ft.name() == feature_name)
            .with_context(|| format!("Fails to find feature of name: {:#?}", feature_name))?;

        Ok(Self {
            workspace,
            path,
            feature,
        })
    }

    pub fn try_new_at_cursor(
        workspace: &'ws Workspace,
        path: &'ws Path,
        cursor: Point,
    ) -> Result<Self> {
        let feature = Self::feature(workspace, path, cursor)?;
        Ok(Self {
            workspace,
            path,
            feature,
        })
    }

    fn class(&self) -> &Class {
        self.workspace.class(self.path).unwrap_or_else(|| {
            unreachable!(
                "fails to find class at path {:#?}, which was already checked.",
                self.path
            )
        })
    }

    fn precondition_add_point(&self) -> Result<Point> {
        let Some(precondition_point) = self.feature.point_end_preconditions() else {
            bail!("The current feature must have an injection point for preconditions.");
        };
        Ok(precondition_point)
    }

    fn postcondition_add_point(&self) -> Result<Point> {
        let Some(postcondition_point) = self.feature.point_end_postconditions() else {
            bail!("The current feature must have an injection point for postconditions.");
        };
        Ok(postcondition_point)
    }

    fn precondition_edit(&self, precondition: Precondition) -> Result<lsp_types::TextEdit> {
        let point = self.precondition_add_point()?;
        let range = Range::new_collapsed(point);

        let new_text = if self.feature.has_precondition() {
            format!("{}", precondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Precondition>::new(precondition, range.clone())
            )
        };
        Ok(lsp_types::TextEdit {
            range: range.try_into()?,
            new_text,
        })
    }

    fn postcondition_edit(&self, postcondition: Postcondition) -> Result<lsp_types::TextEdit> {
        let point = self.postcondition_add_point()?;
        let range = Range::new_collapsed(point);

        let new_text = if self.feature.has_postcondition() {
            format!("{}", postcondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Postcondition>::new(postcondition, range.clone())
            )
        };
        Ok(lsp_types::TextEdit {
            range: range.try_into()?,
            new_text,
        })
    }

    fn routine_specification_edit(
        &self,
        routine_specification: RoutineSpecification,
    ) -> Result<lsp_types::WorkspaceEdit> {
        let precondition = routine_specification.precondition;
        let postcondition = routine_specification.postcondition;
        let pre_edit = self.precondition_edit(precondition)?;
        let post_edit = self.postcondition_edit(postcondition)?;

        Ok(lsp_types::WorkspaceEdit::new(HashMap::from([(
            lsp_types::Url::from_file_path(self.path).map_err(|_| {
                anyhow!(
                    "if on unix path must be absolute. if on windows path must have disk prefix"
                )
            })?,
            vec![pre_edit, post_edit],
        )])))
    }

    fn fix_routine_specifications(
        &self,
        routine_specifications: Vec<RoutineSpecification>,
    ) -> Option<RoutineSpecification> {
        let system_classes = self.system_classes();
        let class = self.class();
        let feature = self.feature;

        let mut parser = Parser::new();
        let fixing_context = FeaturePositionInSystem::new(system_classes, class, feature);

        let specs = routine_specifications
            .into_iter()
            .reduce(|mut acc, mut val| {
                acc.precondition.append(&mut val.precondition);
                acc.postcondition.append(&mut val.postcondition);
                acc
            });

        specs.and_then(|spec| {
            parser
                .fix(spec, &fixing_context)
                .inspect_err(|e| info!("fix refuses routine specification with error: {e:#?}"))
                .ok()
        })
    }
}
