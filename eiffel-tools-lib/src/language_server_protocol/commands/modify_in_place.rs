use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify;
use crate::workspace::Workspace;
use std::error::Error;
use std::ops::ControlFlow;
use std::path::Path;
use std::path::PathBuf;
use tracing::info;
use tracing::warn;

pub enum ModifyInPlaceErrors {
    TimeoutAutoProof {
        class_name: ClassName,
        feature_name: Option<FeatureName>,
        error: Box<dyn Error>,
    },
    TaskJoinError {
        error: Box<dyn Error>,
    },
    RunAutoProofCommand,
}

impl ModifyInPlaceErrors {
    pub fn log(&self) {
        match self {
            ModifyInPlaceErrors::TimeoutAutoProof {
                class_name,
                feature_name,
                ..
            } => {
                let name_entity_under_verification = feature_name.as_ref().map_or_else(
                    || format!("{class_name}"),
                    |name| format!("{class_name}.{name}"),
                );
                warn!(target: "autoproof", "AutoProof times out verifying {name_entity_under_verification}.");
            }
            ModifyInPlaceErrors::TaskJoinError { error } => {
                warn!("Fails to await for AutoProof task because {error:#?}");
            }
            ModifyInPlaceErrors::RunAutoProofCommand => {
                warn!("The LSP fails to run the AutoProof CLI.");
            }
        }
    }
}

async fn update_last_valid_source(
    workspace: &mut Workspace,
    path: PathBuf,
    last_valid_code: &mut Vec<u8>,
) {
    last_valid_code.clone_from(
        &tokio::fs::read(&path)
            .await
            .unwrap_or_else(|e| panic!("Fails to read {path:#?} because {e:#?}.")),
    );
    workspace.reload(path).await
}

async fn reset_source(workspace: &mut Workspace, path: PathBuf, last_valid_code: &mut Vec<u8>) {
    tokio::fs::write(&path, &last_valid_code)
        .await
        .unwrap_or_else(|e| panic!("Fails to read at path {path:#?} because {e:#?}."));
    workspace.reload(path).await
}

async fn failsafe_verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
) -> Result<VerificationResult, ModifyInPlaceErrors> {
    let verification_handle = verify(class_name.clone(), feature_name.cloned(), 60).await;

    let path = workspace.path(&class_name);
    match verification_handle {
        Ok(Ok(Some(verification_result))) => {
            update_last_valid_source(workspace, path.to_path_buf(), last_valid_code).await;
            Ok(verification_result)
        }
        Ok(Ok(None)) => Err(ModifyInPlaceErrors::RunAutoProofCommand),
        Ok(Err(timeout)) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            Err(ModifyInPlaceErrors::TimeoutAutoProof {
                class_name: class_name.clone(),
                feature_name: feature_name.cloned(),
                error: Box::new(timeout),
            })
        }
        Err(fails_to_complete_task) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            Err(ModifyInPlaceErrors::TaskJoinError {
                error: Box::new(fails_to_complete_task),
            })
        }
    }
}

pub async fn verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
) -> ControlFlow<(), Option<String>> {
    let verification =
        failsafe_verification(class_name, feature_name, workspace, last_valid_code).await;
    match verification {
        Ok(VerificationResult::Success) => {
            let success_message = feature_name.map_or_else(
                || format!("The class {class_name} verifies successfully."),
                |name| format!("The feature {class_name}.{name} verifies successfully."),
            );
            info!(target:"autoproof", "{success_message}");

            ControlFlow::Break(())
        }
        Ok(VerificationResult::Failure(error_message)) => {
            ControlFlow::Continue(Some(error_message))
        }
        Err(e) => {
            e.log();
            ControlFlow::Continue(None)
        }
    }
}

pub async fn rewrite_features<'ft, B, I>(path: &Path, features: I)
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    if let Some(content) = rewriting_features(path, features).await {
        let _ = tokio::fs::write(path, content)
            .await
            .inspect_err(|e| warn!("Fails to await rewriting file at {path:#?} with {e:#?}"))
            .ok();
    }
}

async fn rewriting_features<'ft, B, I>(path: &Path, features: I) -> Option<String>
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    tokio::fs::read(path)
        .await
        .inspect_err(|e| warn!("Fails to read {path:#?} because {e:#?}"))
        .ok()
        .and_then(|file_content| {
            String::from_utf8(file_content)
                .inspect_err(|e| {
                    warn!("Fails to convert file at {path:#?} to UFT-8 string because {e:#?}")
                })
                .ok()
        })
        .and_then(|file_content| {
            let mut parser = crate::parser::Parser::new();
            parser
                .class_and_tree_from_source(&file_content)
                .inspect_err(|e| warn!("Fails to parse file at {path:#?} because {e:#?}"))
                .ok()
                .map(|(cl, _)| (file_content, cl))
        })
        .map(|(file_content, class)| {
            let current_features = class.features();
            file_content
                .lines()
                .enumerate()
                .fold(String::new(), |mut acc, (linenum, line)| {
                    on_starting_feature::<B, I>(current_features, features, linenum, line, &mut acc)
                        .or_else(|| {
                            on_surrounding_feature(
                                current_features,
                                features,
                                linenum,
                                line,
                                &mut acc,
                            )
                        })
                        .or_else(|| {
                            on_ending_feature(current_features, features, linenum, line, &mut acc)
                        })
                        .unwrap_or_else(|| format!("{acc}{line}\n"))
                })
        })
}

fn on_starting_feature<'fts, B, I>(
    features: &[Feature],
    new_features: I,
    linenum: usize,
    line: &str,
    acc: &mut String,
) -> Option<String>
where
    B: AsRef<str> + 'fts,
    I: IntoIterator<Item = &'fts (FeatureName, B)>,
{
    features
        .into_iter()
        .find(|ft| ft.range().start.row == linenum)
        .and_then(|ft| {
            matching_new_feature(ft.name(), new_features).map(|(_, new_content)| {
                let range = ft.range();
                let indented_new_content =
                    new_content
                        .as_ref()
                        .lines()
                        .fold(String::new(), |mut acc, line| {
                            if !acc.is_empty() {
                                acc.push('\t');
                            }
                            acc.push_str(line);
                            acc.push('\n');
                            acc
                        });
                let indented_new_content = indented_new_content.trim_end();
                if range.end.row != range.start.row {
                    format!(
                        "{}{}{}",
                        acc,
                        &line[..range.start.column],
                        indented_new_content
                    )
                } else {
                    format!(
                        "{}{}{}{}",
                        acc,
                        &line[..range.start.column],
                        indented_new_content,
                        &line[range.end.column..]
                    )
                }
            })
        })
}

fn on_surrounding_feature<'fts, B>(
    features: &[Feature],
    new_features: impl IntoIterator<Item = &'fts (FeatureName, B)>,
    linenum: usize,
    line: &str,
    acc: &mut String,
) -> std::option::Option<std::string::String>
where
    B: AsRef<str> + 'fts,
{
    features
        .into_iter()
        .find(|ft| {
            let range = ft.range();
            range.start.row < linenum && linenum < range.end.row
        })
        .map(|ft| {
            if matching_new_feature(ft.name(), new_features).is_some() {
                format!("{}", acc)
            } else {
                format!("{}{}\n", acc, line)
            }
        })
}

fn on_ending_feature<'fts, B>(
    features: &[Feature],
    new_features: impl IntoIterator<Item = &'fts (FeatureName, B)>,
    linenum: usize,
    line: &str,
    acc: &mut String,
) -> std::option::Option<std::string::String>
where
    B: AsRef<str> + 'fts,
{
    features
        .into_iter()
        .find(|ft| ft.range().end.row == linenum)
        .map(|ft| {
            if matching_new_feature(ft.name(), new_features).is_some() {
                let range = ft.range();
                format!("{}{}\n", acc, &line[range.end.column..])
            } else {
                format!("{}{}\n", acc, line)
            }
        })
}

fn matching_new_feature<'ft, B>(
    name: &FeatureName,
    features: impl IntoIterator<Item = &'ft (FeatureName, B)>,
) -> Option<&'ft (FeatureName, B)>
where
    B: AsRef<str> + 'ft,
{
    features.into_iter().find(|(ft_name, _)| *ft_name == name)
}
