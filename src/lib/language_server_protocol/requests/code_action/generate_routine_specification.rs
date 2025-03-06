use crate::lib::generators::Generators;
use async_lsp::lsp_types::{CodeAction, CodeActionDisabled, TextEdit, Url, WorkspaceEdit};
use std::collections::HashMap;
use std::sync::Arc;

use crate::lib::code_entities::contract::Fix;
use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use anyhow::anyhow;
use contract::RoutineSpecification;

fn fix_routine_specifications(
    routine_specifications: Vec<RoutineSpecification>,
    system_classes: &[Class],
    current_class: &Class,
    current_feature: &Feature,
) -> Option<RoutineSpecification> {
    let specs = routine_specifications
        .into_iter()
        .reduce(|mut acc, mut val| {
            acc.precondition.append(&mut val.precondition);
            acc.postcondition.append(&mut val.postcondition);
            acc
        });

    specs
        .map(|mut spec| (spec.fix(system_classes, current_class, current_feature)).then_some(spec))
        .flatten()
}

fn file_edits_add_routine_specification(
    file: &ProcessedFile,
    feature: &Feature,
    precondition_insertion_point: Point,
    postcondition_insertion_point: Point,
    routine_specification: RoutineSpecification,
) -> anyhow::Result<WorkspaceEdit> {
    let precondition = routine_specification.precondition;
    let precondition_insertion_range = Range::new_collapsed(precondition_insertion_point);
    let precondition_edit: TextEdit = TextEdit {
        range: precondition_insertion_range.clone().try_into()?,
        new_text: if feature.has_precondition() {
            format!("{}", precondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Precondition>::new(
                    precondition,
                    precondition_insertion_range
                )
            )
        },
    };

    let postcondition = routine_specification.postcondition;
    let postcondition_insertion_range = Range::new_collapsed(postcondition_insertion_point);
    let postcondition_edit: TextEdit = TextEdit {
        range: postcondition_insertion_range.clone().try_into()?,
        new_text: if feature.has_postcondition() {
            format!("{}", postcondition)
        } else {
            format!(
                "{}",
                contract::Block::<contract::Postcondition>::new(
                    postcondition,
                    postcondition_insertion_range
                )
            )
        },
    };

    Ok(WorkspaceEdit::new(HashMap::from([(
        Url::from_file_path(file.path()).map_err(|e| {
            anyhow!("if on unix path must be absolute. if on windows path must have disk prefix")
        })?,
        vec![precondition_edit, postcondition_edit],
    )])))
}

async fn routine_specifications_as_workspace_edit(
    file: &ProcessedFile,
    feature: &Feature,
    generators: &Generators,
    system_classes: &[Class],
) -> anyhow::Result<WorkspaceEdit> {
    let more_routine_specs = generators
        .more_routine_specifications(feature, file, system_classes)
        .await?;

    let fixed =
        fix_routine_specifications(more_routine_specs, system_classes, file.class(), feature);
    let Some(spec) = fixed else {
        return Err(anyhow!("No valid specifications were generated."));
    };
    let Some(precondition_injection_point) = feature.point_end_preconditions() else {
        return Err(anyhow!(
            "The current feature must have an injection point for preconditions."
        ));
    };
    let Some(postcondition_injection_point) = feature.point_end_postconditions() else {
        return Err(anyhow!(
            "The current feature must have an injection point for postconditions."
        ));
    };
    file_edits_add_routine_specification(
        file,
        feature,
        precondition_injection_point.clone(),
        postcondition_injection_point.clone(),
        spec,
    )
}

pub async fn code_action(
    generators: &Generators,
    file: Option<&ProcessedFile>,
    system_classes: &[Class],
    cursor_point: &Point,
) -> CodeAction {
    let (edit, disabled) = match file {
        Some(file) => match file.feature_around_point(&cursor_point) {
            Some(feature) => {
                match routine_specifications_as_workspace_edit(
                    file,
                    &feature,
                    generators,
                    &system_classes,
                )
                .await
                {
                    Ok(edit) => (Some(edit), None),
                    Err(e) => (
                        None,
                        Some(CodeActionDisabled {
                            reason: format!("{e:#?}"),
                        }),
                    ),
                }
            }
            None => (
                None,
                Some(CodeActionDisabled {
                    reason: String::from("The cursor must be inside a feature."),
                }),
            ),
        },
        None => (
            None,
            Some(CodeActionDisabled {
                reason: "The current file must be parsed.".to_string(),
            }),
        ),
    };

    CodeAction {
        title: String::from("Add contracts to current routine"),
        kind: None,
        diagnostics: None,
        edit,
        command: None,
        is_preferred: Some(false),
        disabled,
        data: None,
    }
}
