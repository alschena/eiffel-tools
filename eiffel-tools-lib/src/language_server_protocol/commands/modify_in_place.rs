use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify;
use crate::parser;
use crate::workspace::Workspace;
use std::ops::ControlFlow;
use std::path::Path;
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;
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
    let path = workspace.path(class_name);
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
            info!("The LSP fails to run the AutoProof CLI.");
            ControlFlow::Break(())
        }
        Ok(Err(_timeout)) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            info!(target: "autoproof", "AutoProof times out verifying {entity_under_verification}.");
            ControlFlow::Continue(None)
        }
        Err(fails_to_complete_task) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            info!("Fails to await for AutoProof task because {fails_to_complete_task:#?}");
            ControlFlow::Continue(None)
        }
    }
}

pub async fn rewrite_features<'ft, B, I>(path: &Path, features: I)
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    let maybe_rewrite_handle = tokio::fs::read(path)
        .await
        .inspect_err(|e| warn!("Fails to await reading {path:#?} before rewrite because {e:#?}"))
        .ok()
        .and_then(move |ref content| {
            str::from_utf8(content)
                .inspect_err(|e| warn!("Fails to convert file content to UFT-8 because {e:#?}"))
                .ok()
                .and_then(|initial_content| rewriting_features(initial_content, features))
        })
        .map(move |new_file| {
            let path = path.to_owned();
            tokio::spawn(tokio::fs::write(path, new_file))
        });

    if let Some(rewrite_handle) = maybe_rewrite_handle {
        match rewrite_handle.await {
            Ok(Err(e)) => {
                warn!("Fails to rewrite fetures because {e:#?}")
            }
            Err(e) => {
                warn!("Fails to await the rewriting of features because {e:#?}.")
            }
            Ok(Ok(())) => {}
        }
    }
}

fn rewriting_features<'ft, B, I>(initial_source: &str, features: I) -> Option<String>
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    parser::Parser::default()
        .class_and_tree_from_source(initial_source)
        .inspect_err(|e| warn!("Fails to parse file rewriting feature because {e:#?}"))
        .ok()
        .map(|(cl, _)| (initial_source, cl))
        .map(|(initial_source, class)| {
            let current_features = class.features();
            initial_source
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
        .iter()
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
        .iter()
        .find(|ft| {
            let range = ft.range();
            range.start.row < linenum && linenum < range.end.row
        })
        .map(|ft| {
            if matching_new_feature(ft.name(), new_features).is_some() {
                acc.to_string()
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
        .iter()
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

pub async fn clear_comments(path: &Path) {
    match tokio::fs::read(path).await {
        Ok(mut content) => {
            remove_comments(&mut content);
            match tokio::fs::write(path, content).await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Fails to write to {path:#?} because {e:#?}");
                }
            }
        }
        Err(e) => {
            warn!("Fails to read {path:#?} because {e:#?}");
        }
    }
}

fn ordered_comment_ranges<S: AsRef<[u8]>>(content: &S) -> Vec<tree_sitter::Range> {
    let mut parser = parser::Parser::default();
    let parsed_source = parser
        .parse(content.as_ref())
        .unwrap_or_else(|_| panic!("Should parse file to extract comments."));

    let query = tree_sitter::Query::new(
        &tree_sitter_eiffel::LANGUAGE.into(),
        "[(comment) (header_comment)] @comment",
    )
    .unwrap_or_else(|e| {
        panic!("Should create the query for comment nodes, instead fails with {e:#?}.")
    });

    let capture_index = query
        .capture_index_for_name("comment")
        .unwrap_or_else(|| panic!("Should capture nodes of `comment` type."));

    let mut query_cursor = tree_sitter::QueryCursor::new();

    let comments_matches = query_cursor.matches(
        &query,
        parsed_source.tree().root_node(),
        parsed_source.source(),
    );

    let mut comment_ranges = comments_matches.fold(Vec::new(), |mut acc, mtc| {
        acc.extend(
            mtc.nodes_for_capture_index(capture_index)
                .map(|node| node.range()),
        );
        acc
    });

    comment_ranges.sort_by(|lhs, rhs| lhs.start_byte.cmp(&rhs.start_byte));
    comment_ranges
}

fn remove_comments(content: &mut Vec<u8>) {
    let mut negative_offset = 0;
    for range in ordered_comment_ranges(&content) {
        let mut start = range.start_byte - negative_offset;
        let end = range.end_byte - negative_offset;

        content.drain(start..end);

        let (maybe_num_bytes_to_trim_from_end, should_add_newline) = content[0..start]
            .iter()
            .rev()
            .enumerate()
            .find(|(_, char)| **char != b' ' && **char != b'\t')
            .map_or_else(
                || (None, false),
                |(back_index, char)| (Some(back_index), *char != b'\n'),
            );

        if let Some(offset_back_start) = maybe_num_bytes_to_trim_from_end {
            content.drain(start - offset_back_start..start);
            start -= offset_back_start;
        }

        if should_add_newline {
            content.insert(start, b'\n');
            start += 1;
        }

        negative_offset += end - start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    const COMMENTED_NEWTEXT: &'static str = r#"
    -- This is a comment
class A -- This is a comment
    -- This is a comment
feature -- This is a comment
    -- This is a comment
  x: INTEGER -- This is a comment
  -- This is a comment
  y: INTEGER -- This is a comment
  -- This is a comment
end -- This is a comment 
            "#;

    fn initialize_file_with_oldtext(file: &ChildPath) -> Workspace {
        file.write_str(OLDTEXT)
            .expect("Fails to initialize temporary file for testing.");

        let mut parser = Parser::default();
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

    #[tokio::test]
    async fn test_remove_comments() {
        let mut text: Vec<u8> = COMMENTED_NEWTEXT.as_bytes().to_vec();
        super::remove_comments(&mut text);
        let human_readable_output = str::from_utf8(&text).expect("Should convert text to UFT-8");

        let equal_upto_trimming_lines = human_readable_output
            .lines()
            .zip(NEWTEXT.lines())
            .all(|(output, oracle)| output.trim() == oracle.trim());

        assert!(
            equal_upto_trimming_lines,
            "OUTPUT: {human_readable_output}\nORACLE: {NEWTEXT}"
        );
    }
}
