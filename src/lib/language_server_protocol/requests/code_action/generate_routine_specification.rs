use crate::lib::code_entities::contract::Fix;
use crate::lib::code_entities::prelude::*;
use crate::lib::generators::Generators;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use async_lsp::lsp_types::{CodeAction, CodeActionDisabled, TextEdit, Url, WorkspaceEdit};
use contract::Postcondition;
use contract::Precondition;
use contract::RoutineSpecification;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Clone)]
pub struct SourceGenerationContext<'ws> {
    workspace: &'ws Workspace,
    file: &'ws ProcessedFile,
    cursor: Point,
}
impl Deref for SourceGenerationContext<'_> {
    type Target = Workspace;

    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}

impl SourceGenerationContext<'_> {
    pub fn new<'ws>(
        workspace: &'ws Workspace,
        file: &'ws ProcessedFile,
        cursor: Point,
    ) -> SourceGenerationContext<'ws> {
        SourceGenerationContext {
            workspace,
            file,
            cursor,
        }
    }
    fn class(&self) -> &Class {
        self.file.class()
    }

    fn feature(&self) -> anyhow::Result<&Feature> {
        let cursor = self.cursor;
        self.file
            .feature_around_point(cursor)
            .with_context(|| "cursor is not around feature.")
    }

    fn precondition_add_point(&self) -> anyhow::Result<Point> {
        let feature = self.feature()?;
        let Some(precondition_point) = feature.point_end_preconditions() else {
            bail!("The current feature must have an injection point for preconditions.");
        };
        Ok(precondition_point)
    }

    fn postcondition_add_point(&self) -> anyhow::Result<Point> {
        let feature = self.feature()?;
        let Some(postcondition_point) = feature.point_end_postconditions() else {
            bail!("The current feature must have an injection point for postconditions.");
        };
        Ok(postcondition_point)
    }

    fn precondition_edit(&self, precondition: Precondition) -> anyhow::Result<TextEdit> {
        let point = self.precondition_add_point()?;
        let range = Range::new_collapsed(point);
        let feature = self.feature()?;

        let new_text = if feature.has_precondition() {
            format!("{}", precondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Precondition>::new(precondition, range.clone())
            )
        };
        Ok(TextEdit {
            range: range.try_into()?,
            new_text,
        })
    }

    fn postcondition_edit(&self, postcondition: Postcondition) -> anyhow::Result<TextEdit> {
        let point = self.postcondition_add_point()?;
        let range = Range::new_collapsed(point);
        let feature = self.feature()?;

        let new_text = if feature.has_postcondition() {
            format!("{}", postcondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Postcondition>::new(postcondition, range.clone())
            )
        };
        Ok(TextEdit {
            range: range.try_into()?,
            new_text,
        })
    }

    fn routine_specification_edit(
        &self,
        routine_specification: RoutineSpecification,
    ) -> anyhow::Result<WorkspaceEdit> {
        let precondition = routine_specification.precondition;
        let postcondition = routine_specification.postcondition;
        let pre_edit = self.precondition_edit(precondition)?;
        let post_edit = self.postcondition_edit(postcondition)?;

        Ok(WorkspaceEdit::new(HashMap::from([(
            Url::from_file_path(self.file.path()).map_err(|_| {
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
        let feature = self.feature().ok()?;

        let specs = routine_specifications
            .into_iter()
            .reduce(|mut acc, mut val| {
                acc.precondition.append(&mut val.precondition);
                acc.postcondition.append(&mut val.postcondition);
                acc
            });

        specs
            .map(|mut spec| (spec.fix(&system_classes, class, feature)).then_some(spec))
            .flatten()
    }

    async fn generate_edits(&self, generators: &Generators) -> anyhow::Result<WorkspaceEdit> {
        let file = self.file;
        let feature = self.feature()?;
        let system_classes = self.system_classes();
        let more_routine_spec = generators
            .more_routine_specifications(feature, file, &system_classes)
            .await
            .inspect(|val| eprintln!("mVAL:\t{val:#?}"))
            .inspect_err(|e| eprintln!("mERR:\t{e:#?}"))?;
        let fixed = self
            .fix_routine_specifications(more_routine_spec)
            .with_context(|| "fix routine specifications.")
            .inspect(|val| eprintln!("VAL:\t{val:#?}"))
            .inspect_err(|e| eprintln!("ERR:\t{e:#?}"))?;
        self.routine_specification_edit(fixed)
            .inspect(|val| eprintln!("val:\t{val:#?}"))
            .inspect_err(|e| eprintln!("err:\t{e:#?}"))
    }

    pub async fn code_action(&self, generators: &Generators) -> CodeAction {
        let title = String::from("Add contracts to current routine");
        let kind = None;
        let diagnostics = None;
        let command = None;
        let is_preferred = Some(false);
        let data = None;
        match self.generate_edits(generators).await {
            Ok(text) => CodeAction {
                title,
                kind,
                diagnostics,
                edit: Some(text),
                command,
                is_preferred,
                disabled: None,
                data,
            },
            Err(e) => CodeAction {
                title,
                kind,
                diagnostics,
                edit: None,
                command,
                is_preferred,
                disabled: Some(CodeActionDisabled {
                    reason: format!("{e}"),
                }),
                data,
            },
        }
    }
}
