use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::generators::Generators;
use crate::parser::Parser;
use crate::workspace::Workspace;
use std::path::Path;
use tracing::info;
use tracing::warn;

pub async fn fix_routine_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
    feature: &Feature,
) {
    let path = workspace.path(class_name).to_path_buf();
    let mut last_valid_code = tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("fails to read at path {:#?} with {:#?}", &path, e));

    for number_of_tries in 0..10 {
        let feature_name = feature.name();
        let verification = super::modify_in_place::failsafe_verification(
            workspace,
            class_name.to_owned(),
            Some(feature_name.to_owned()),
            &mut last_valid_code,
        )
        .await;

        match verification {
            Ok(VerificationResult::Success) => {
                info!(
                    "The feature {}.{} verifies successfully at try #{}.",
                    class_name, feature_name, number_of_tries
                );
                break;
            }
            Ok(VerificationResult::Failure(error_message)) => {
                if let Some((ft_name, body)) = generators
                    .routine_fixes(&workspace, &path, &feature_name, error_message)
                    .await
                {
                    rewrite_feature(&path, (ft_name, &body)).await;
                }
            }
            Err(e) => e.log(),
        }
    }
}

async fn read_file(path: &Path) -> Option<String> {
    tokio::fs::read(path)
        .await
        .inspect_err(|e| warn!("fails to read {path:#?} with {e:#?}"))
        .ok()
        .and_then(|content| {
            String::from_utf8(content)
                .inspect_err(|e| warn!("fails to convert current file to UTF-8 string with {e:#?}"))
                .ok()
        })
}

async fn rewrite_feature(path: &Path, new_feature: (&FeatureName, &str)) {
    let Some(current_file) = read_file(path).await else {
        return;
    };

    let mut parser = Parser::new();
    let Ok((class, _)) = parser
        .class_and_tree_from_source(&current_file)
        .inspect_err(|e| warn!("fails to parse current file {path:#?} with {e:#?}"))
    else {
        return;
    };

    let current_features: Vec<_> = class
        .features()
        .iter()
        .map(|ft| (ft.name(), ft.range()))
        .collect();

    let starting_current_feature = |linenum| {
        current_features
            .iter()
            .find(|(_, range)| range.start.row == linenum)
    };

    let surrounding_current_feature = |linenum| {
        current_features
            .iter()
            .find(|(_, range)| (range.start.row < linenum && linenum < range.end.row))
    };

    let ending_current_feature = |linenum| {
        current_features
            .iter()
            .find(|(_, range)| range.end.row == linenum)
    };

    let matching_new_feature = |&feature_name| {
        let (ft_name, _) = new_feature;
        (ft_name == feature_name).then_some(new_feature)
    };

    let on_starting_feature = |acc: &mut String, linenum, line: &str| {
        starting_current_feature(linenum).and_then(|(name, range)| {
            matching_new_feature(name).map(|(_, content)| {
                if range.end.row != range.start.row {
                    format!("{}{}{}\n", acc, &line[..range.start.column], content.trim())
                } else {
                    format!(
                        "{}{}{}{}\n",
                        acc,
                        &line[..range.start.column],
                        content,
                        &line[range.end.column..]
                    )
                }
            })
        })
    };

    let on_surrounding_feature = |acc: &mut String, linenum, line: &str| {
        surrounding_current_feature(linenum).map(|(name, _)| {
            if matching_new_feature(name).is_some() {
                format!("{}", acc)
            } else {
                format!("{}{}\n", acc, line)
            }
        })
    };

    let on_ending_feature = |acc: &mut String, linenum, line: &str| {
        ending_current_feature(linenum).map(|(name, range)| {
            if matching_new_feature(name).is_some() {
                format!("{}{}\n", acc, &line[range.end.column..])
            } else {
                format!("{}{}\n", acc, line)
            }
        })
    };

    let new_file =
        current_file
            .lines()
            .enumerate()
            .fold(String::new(), |mut acc, (linenum, line)| {
                on_starting_feature(&mut acc, linenum, line)
                    .or_else(|| on_surrounding_feature(&mut acc, linenum, line))
                    .or_else(|| on_ending_feature(&mut acc, linenum, line))
                    .unwrap_or_else(|| format!("{}{}\n", acc, line))
            });

    if !new_file.is_empty() {
        let _ = tokio::fs::write(path, new_file)
            .await
            .inspect_err(|e| warn!("fails to await for rewriting file at {path:#?} with {e:#?}"))
            .ok();
    }
}
