use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify;
use crate::workspace::Workspace;
use std::ops::ControlFlow;
use std::path::Path;
use std::path::PathBuf;
use tracing::info;
use tracing::warn;

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

pub async fn verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
) -> ControlFlow<(), Option<String>> {
    let path = workspace.path(&class_name);
    let entity_under_verification = feature_name.map_or_else(
        || format!("{class_name}"),
        |name| format!("{class_name}.{name}"),
    );

    let verification_handle = verify(class_name.clone(), feature_name.cloned(), 60).await;

    match verification_handle {
        Ok(Ok(Some(VerificationResult::Success))) => {
            update_last_valid_source(workspace, path.to_path_buf(), last_valid_code).await;
            info!(target:"autoproof", "AutoProof verifies {entity_under_verification} successfully.");

            ControlFlow::Break(())
        }
        Ok(Ok(Some(VerificationResult::Failure(error_message)))) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            info!(target: "autoproof", "AutoProof fails to verify {entity_under_verification}.");
            ControlFlow::Continue(Some(error_message))
        }
        Ok(Ok(None)) => {
            warn!("The LSP fails to run the AutoProof CLI.");
            ControlFlow::Break(())
        }
        Ok(Err(_timeout)) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            warn!(target: "autoproof", "AutoProof times out verifying {entity_under_verification}.");
            ControlFlow::Continue(None)
        }
        Err(fails_to_complete_task) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            warn!("Fails to await for AutoProof task because {fails_to_complete_task:#?}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_server_protocol::commands::modify_in_place::reset_source;
    use crate::parser::Parser;
    use crate::workspace::Workspace;
    use assert_fs::TempDir;
    use assert_fs::fixture::ChildPath;
    use assert_fs::prelude::*;

    const OLDTEXT: &'static str = r#"
class A
feature
  x: INTEGER
end
            "#;

    const NEWTEXT: &'static str = r#"
class A
feature
  x: INTEGER
  y: INTEGER
end
            "#;

    fn initialize_file_with_oldtext(file: &ChildPath) -> Workspace {
        file.write_str(OLDTEXT)
            .expect("Fails to initialize temporary file for testing.");

        let mut parser = Parser::new();
        let (cl, tr) = parser
            .class_and_tree_from_source(OLDTEXT)
            .expect(stringify!("Fails to parse test class at {}", file!()));
        let mut ws = Workspace::new();
        ws.add_file((cl, file.to_path_buf(), tr));

        ws
    }

    #[tokio::test]
    async fn test_update_last_valid_source() {
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("to_update_last_valid_source");

        let mut ws = initialize_file_with_oldtext(&file);

        file.write_str(NEWTEXT)
            .expect("fails to write `NEWTEXT` on file");
        let mut last_valid_code = OLDTEXT.as_bytes().to_owned();

        update_last_valid_source(&mut ws, file.to_path_buf(), &mut last_valid_code).await;

        assert_eq!(ws.class(file.path()).map(|cl| cl.features().len()), Some(2));
        assert_eq!(last_valid_code, NEWTEXT.as_bytes().to_owned());
    }

    #[tokio::test]
    async fn test_reset_source() {
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("to_reset_source");

        let mut ws = initialize_file_with_oldtext(&file);

        file.write_str(NEWTEXT)
            .expect("fails to write `NEWTEXT` on file");
        let mut last_valid_code = OLDTEXT.as_bytes().to_owned();

        reset_source(&mut ws, file.to_path_buf(), &mut last_valid_code).await;

        assert_eq!(ws.class(file.path()).map(|cl| cl.features().len()), Some(1));
        assert_eq!(last_valid_code, OLDTEXT.as_bytes().to_owned());
    }
}
