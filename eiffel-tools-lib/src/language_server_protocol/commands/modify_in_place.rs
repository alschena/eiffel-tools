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

// Thread-local storage to track the previous verification handle for cancellation
thread_local! {
    static PREVIOUS_VERIFICATION_HANDLE: std::cell::RefCell<Option<tokio::task::JoinHandle<Result<Option<crate::eiffelstudio_cli::VerificationResult>, tokio::time::error::Elapsed>>>> = std::cell::RefCell::new(None);
}

pub async fn verification(
    class_name: &ClassName,
    feature_name: Option<&FeatureName>,
    workspace: &mut Workspace,
    last_valid_code: &mut Vec<u8>,
    attempt_number: Option<u32>,
    verbose: bool,
) -> ControlFlow<(), Option<String>> {
    let path = workspace.path(class_name);
    let entity_under_verification = feature_name.map_or_else(
        || format!("{class_name}"),
        |name| format!("{class_name}.{name}"),
    );

    // Abort any previous verification handle before starting a new one
    // This ensures we don't have multiple AutoProof processes running concurrently for the same feature
    PREVIOUS_VERIFICATION_HANDLE.with(|prev_handle| {
        if let Some(handle) = prev_handle.borrow_mut().take() {
            handle.abort();
            info!(
                target: "autoproof",
                "Aborted previous AutoProof verification for {entity_under_verification} before starting new attempt"
            );
        }
    });

    if verbose {
        if let Some(attempt) = attempt_number {
            eprintln!("Starting verification attempt #{} for {}", attempt, entity_under_verification);
        } else {
            eprintln!("Starting verification attempt for {}", entity_under_verification);
        }
    }
    let verification_handle = verify(class_name.clone(), feature_name.cloned(), 60, verbose);
    
    // Note: We can't store the current handle for future cancellation because JoinHandle doesn't implement Clone
    // and we need to await it to get the result. However, we've already aborted any previous handle above,
    // and the verify function now kills processes by PID even after completion, which should handle
    // any lingering processes from EiffelStudio bugs.
    let verification_result = verification_handle.await;

    match verification_result {
        Ok(Ok(Some(VerificationResult::Success))) => {
            update_last_valid_source(workspace, path.to_path_buf(), last_valid_code).await;
            if verbose {
                if let Some(attempt) = attempt_number {
                    eprintln!("[Attempt #{}] Verification succeeded for {}", attempt, entity_under_verification);
                } else {
                    eprintln!("Verification succeeded for {}", entity_under_verification);
                }
            }
            info!(target:"autoproof", "AutoProof verifies {entity_under_verification} successfully.");

            ControlFlow::Break(())
        }
        Ok(Ok(Some(VerificationResult::Failure(error_message)))) => {
            reset_source(workspace, path.to_path_buf(), last_valid_code).await;
            if verbose {
                if let Some(attempt) = attempt_number {
                    eprintln!("[Attempt #{}] Verification failed for {}:\n{}", attempt, entity_under_verification, error_message);
                } else {
                    eprintln!("Verification failed for {}:\n{}", entity_under_verification, error_message);
                }
            }
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

pub async fn rewrite_feature_bodies<'ft, B, I>(path: &Path, feature_bodies: I)
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
                .and_then(|initial_content| rewriting_feature_bodies(initial_content, feature_bodies))
        })
        .map(move |new_file| {
            let path = path.to_owned();
            tokio::spawn(tokio::fs::write(path, new_file))
        });

    if let Some(rewrite_handle) = maybe_rewrite_handle {
        match rewrite_handle.await {
            Ok(Err(e)) => {
                warn!("Fails to rewrite feature bodies because {e:#?}")
            }
            Err(e) => {
                warn!("Fails to await the rewriting of feature bodies because {e:#?}.")
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

fn rewriting_feature_bodies<'ft, B, I>(initial_source: &str, feature_bodies: I) -> Option<String>
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    parser::Parser::default()
        .class_and_tree_from_source(initial_source)
        .inspect_err(|e| warn!("Fails to parse file rewriting feature bodies because {e:#?}"))
        .ok()
        .map(|(cl, _)| (initial_source, cl))
        .map(|(initial_source, class)| {
            let current_features = class.features();
            initial_source
                .lines()
                .enumerate()
                .fold(String::new(), |mut acc, (linenum, line)| {
                    on_starting_body::<B, I>(current_features, feature_bodies, linenum, line, &mut acc)
                        .or_else(|| {
                            on_surrounding_body(
                                current_features,
                                feature_bodies,
                                linenum,
                                line,
                                &mut acc,
                            )
                        })
                        .or_else(|| {
                            on_ending_body(current_features, feature_bodies, linenum, line, &mut acc)
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

fn on_starting_body<'fts, B, I>(
    features: &[Feature],
    new_bodies: I,
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
        .find(|ft| {
            ft.body_range()
                .map(|body_range| body_range.start.row == linenum)
                .unwrap_or(false)
        })
        .and_then(|ft| {
            matching_new_feature(ft.name(), new_bodies).and_then(|(_, new_body)| {
                ft.body_range().map(|body_range| {
                    // Skip the "do" keyword (2 characters) when replacing
                    let mut body_range = body_range.clone();
                    let do_end_column = body_range.start.column + 2;
                    body_range.start.column += 2;
                    let indented_new_body =
                        new_body
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
                    let indented_new_body = indented_new_body.trim_end();
                    if body_range.end.row != body_range.start.row {
                        // Body spans multiple lines, add newline after "do"
                        format!(
                            "{}{}\n{}",
                            acc,
                            &line[..do_end_column],
                            indented_new_body
                        )
                    } else {
                        // Body is on same line as "do", need to add newline after "do"
                        format!(
                            "{}{}\n{}{}",
                            acc,
                            &line[..do_end_column],
                            indented_new_body,
                            &line[body_range.end.column..]
                        )
                    }
                })
            })
        })
}

fn on_surrounding_body<'fts, B>(
    features: &[Feature],
    new_bodies: impl IntoIterator<Item = &'fts (FeatureName, B)>,
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
            ft.body_range()
                .map(|body_range| {
                    body_range.start.row < linenum && linenum < body_range.end.row
                })
                .unwrap_or(false)
        })
        .map(|ft| {
            if matching_new_feature(ft.name(), new_bodies).is_some() {
                acc.to_string()
            } else {
                format!("{}{}\n", acc, line)
            }
        })
}

fn on_ending_body<'fts, B>(
    features: &[Feature],
    new_bodies: impl IntoIterator<Item = &'fts (FeatureName, B)>,
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
            ft.body_range()
                .map(|body_range| body_range.end.row == linenum)
                .unwrap_or(false)
        })
        .map(|ft| {
            if matching_new_feature(ft.name(), new_bodies).is_some() {
                ft.body_range()
                    .map(|body_range| format!("{}{}\n", acc, &line[body_range.end.column..]))
                    .unwrap_or_else(|| format!("{}{}\n", acc, line))
            } else {
                format!("{}{}\n", acc, line)
            }
        })
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

    const FEATURE_WITH_CONTRACTS: &'static str = r#"
class TEST_CLASS
feature
    compute (x: INTEGER): INTEGER
        require
            x >= 0
            x < 100
        do
            Result := x * 2
        ensure
            Result >= 0
            Result = x * 2
        end
end
            "#;

    const FEATURE_WITH_NEW_BODY: &'static str = r#"
class TEST_CLASS
feature
    compute (x: INTEGER): INTEGER
        require
            x >= 0
            x < 100
        do
            Result := x * 3
        ensure
            Result >= 0
            Result = x * 2
        end
end
            "#;

    #[tokio::test]
    async fn test_rewrite_feature_bodies_preserves_contracts() {
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_preserve_contracts");

        // Write initial feature with contracts
        file.write_str(FEATURE_WITH_CONTRACTS)
            .expect("Fails to write initial feature with contracts");

        // Parse and get the feature
        let mut parser = Parser::default();
        let (class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITH_CONTRACTS)
            .expect("Fails to parse test class");
        let mut ws = Workspace::new();
        ws.add_file((class.clone(), file.to_path_buf(), tree));

        let feature = class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Verify initial contracts exist
        assert!(
            feature.has_precondition(),
            "Feature should have precondition"
        );
        assert!(
            feature.has_postcondition(),
            "Feature should have postcondition"
        );

        // Get the original body
        let original_body = feature
            .body_source_unchecked(FEATURE_WITH_CONTRACTS)
            .expect("Should extract original body");

        // Replace only the body (simulating what fix_routine_in_place does)
        let new_body = "Result := x * 3";
        rewrite_feature_bodies(file.path(), &[(feature.name().to_owned(), new_body)]).await;

        // Reload workspace
        ws.reload(file.to_path_buf()).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Parse the modified content
        let (modified_class, _) = parser
            .class_and_tree_from_source(&modified_content)
            .expect("Should parse modified class");

        let modified_feature = modified_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature after modification");

        // Verify contracts are still present
        assert!(
            modified_feature.has_precondition(),
            "Precondition should be preserved after body replacement"
        );
        assert!(
            modified_feature.has_postcondition(),
            "Postcondition should be preserved after body replacement"
        );

        // Verify the body was actually changed
        let modified_body = modified_feature
            .body_source_unchecked(modified_content.as_str())
            .expect("Should extract modified body");

        assert_ne!(
            original_body.trim(),
            modified_body.trim(),
            "Body should be different after replacement"
        );

        // Verify the new body is present
        assert!(
            modified_body.contains("x * 3"),
            "Modified body should contain the new computation"
        );
        assert!(
            !modified_body.contains("x * 2"),
            "Modified body should not contain the old computation"
        );

        // Verify contracts are unchanged by checking the source
        assert!(
            modified_content.contains("require"),
            "Modified content should contain require clause"
        );
        assert!(
            modified_content.contains("x >= 0"),
            "Modified content should preserve precondition clause"
        );
        assert!(
            modified_content.contains("x < 100"),
            "Modified content should preserve precondition clause"
        );
        assert!(
            modified_content.contains("ensure"),
            "Modified content should contain ensure clause"
        );
        assert!(
            modified_content.contains("Result >= 0"),
            "Modified content should preserve postcondition clause"
        );
        assert!(
            modified_content.contains("Result = x * 2"),
            "Modified content should preserve postcondition clause"
        );

        // Verify the file matches our expected output (allowing for whitespace differences)
        let expected_lines: Vec<&str> = FEATURE_WITH_NEW_BODY.lines().collect();
        let actual_lines: Vec<&str> = modified_content.lines().collect();
        
        // Compare line by line, ignoring leading/trailing whitespace
        for (expected, actual) in expected_lines.iter().zip(actual_lines.iter()) {
            assert_eq!(
                expected.trim(),
                actual.trim(),
                "Line mismatch. Expected: '{}', Actual: '{}'",
                expected,
                actual
            );
        }
    }

    const FEATURE_WITH_ORIGINAL_CONTRACTS: &'static str = r#"
class TEST_CLASS
feature
    add (x, y: INTEGER): INTEGER
        require
            x >= 0
            y >= 0
        do
            Result := x + y
        ensure
            Result >= x
            Result >= y
        end
end
            "#;

    const LLM_FULL_FEATURE_WITH_DIFFERENT_CONTRACTS: &'static str = r#"
class TEST_CLASS
feature
    add (x, y: INTEGER): INTEGER
        require
            x > 0
            y > 0
        do
            Result := x + y + 1
        ensure
            Result > x + y
        end
end
            "#;


    #[tokio::test]
    async fn test_llm_returns_full_feature_with_contracts_only_body_replaced() {
        // This test simulates the case where LLM returns a full feature including contracts,
        // but we extract only the body and replace it, preserving original contracts.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_llm_full_feature");

        // Write initial feature with original contracts
        file.write_str(FEATURE_WITH_ORIGINAL_CONTRACTS)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITH_ORIGINAL_CONTRACTS)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "add")
            .expect("Should find add feature");

        // Simulate LLM returning a full feature with different contracts
        // (This is what fixed_routine_src returns: (Feature, full_feature_source))
        let (llm_feature, llm_full_source) = {
            let (llm_class, _) = parser
                .class_and_tree_from_source(LLM_FULL_FEATURE_WITH_DIFFERENT_CONTRACTS)
                .expect("Fails to parse LLM-generated feature");
            let llm_feat = llm_class
                .features()
                .iter()
                .find(|f| f.name() == "add")
                .expect("Should find LLM-generated feature")
                .clone();
            (llm_feat, LLM_FULL_FEATURE_WITH_DIFFERENT_CONTRACTS.to_string())
        };

        // Simulate what fix_routine_in_place does: extract only the body
        let body_only = llm_feature
            .body_source_unchecked(llm_full_source.as_str())
            .expect("Should extract body from LLM-generated full feature");

        // Verify the extracted body doesn't include contracts
        assert!(
            !body_only.contains("require"),
            "Extracted body should not contain require clause"
        );
        assert!(
            !body_only.contains("ensure"),
            "Extracted body should not contain ensure clause"
        );
        assert!(
            body_only.contains("x + y + 1"),
            "Extracted body should contain the new computation"
        );

        // Replace only the body (this is what rewrite_feature_bodies does)
        rewrite_feature_bodies(file.path(), &[(original_feature.name().to_owned(), body_only)]).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Parse the modified content
        let (modified_class, _) = parser
            .class_and_tree_from_source(&modified_content)
            .expect("Should parse modified class");

        let modified_feature = modified_class
            .features()
            .iter()
            .find(|f| f.name() == "add")
            .expect("Should find add feature after modification");

        // Verify ORIGINAL contracts are preserved (not LLM's contracts)
        assert!(
            modified_feature.has_precondition(),
            "Precondition should be preserved"
        );
        assert!(
            modified_feature.has_postcondition(),
            "Postcondition should be preserved"
        );

        // Verify original precondition clauses are still there
        assert!(
            modified_content.contains("x >= 0"),
            "Original precondition clause 'x >= 0' should be preserved"
        );
        assert!(
            modified_content.contains("y >= 0"),
            "Original precondition clause 'y >= 0' should be preserved"
        );
        // Verify LLM's different precondition clauses are NOT present
        assert!(
            !modified_content.contains("x > 0"),
            "LLM's precondition clause 'x > 0' should NOT be present"
        );
        assert!(
            !modified_content.contains("y > 0"),
            "LLM's precondition clause 'y > 0' should NOT be present"
        );

        // Verify original postcondition clauses are still there
        assert!(
            modified_content.contains("Result >= x"),
            "Original postcondition clause 'Result >= x' should be preserved"
        );
        assert!(
            modified_content.contains("Result >= y"),
            "Original postcondition clause 'Result >= y' should be preserved"
        );
        // Verify LLM's different postcondition clause is NOT present
        assert!(
            !modified_content.contains("Result > x + y"),
            "LLM's postcondition clause 'Result > x + y' should NOT be present"
        );

        // Verify the body was updated with LLM's body
        let modified_body = modified_feature
            .body_source_unchecked(modified_content.as_str())
            .expect("Should extract modified body");

        assert!(
            modified_body.contains("x + y + 1"),
            "Modified body should contain LLM's computation. Body was: {:?}",
            modified_body
        );
        // Note: The body might contain "x + y" as part of "x + y + 1", so we check more specifically
        let body_trimmed = modified_body.trim();
        assert!(
            body_trimmed.contains("+ 1") || body_trimmed.contains("x + y + 1"),
            "Modified body should contain '+ 1' or 'x + y + 1'. Body was: {:?}",
            modified_body
        );

        // Verify the key aspects: contracts preserved and body updated
        // (We don't do exact line-by-line comparison due to potential whitespace differences)
        // The important thing is that contracts are preserved and body is updated, which we've already verified above
    }

    #[tokio::test]
    async fn test_llm_returns_just_body_only_body_replaced() {
        // This test simulates the case where LLM returns just the body (no contracts),
        // which should work directly with rewrite_feature_bodies.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_llm_just_body");

        // Write initial feature with contracts
        file.write_str(FEATURE_WITH_ORIGINAL_CONTRACTS)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITH_ORIGINAL_CONTRACTS)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "add")
            .expect("Should find add feature");

        // Simulate LLM returning just the body (no contracts, no feature signature)
        // This is what body_source_unchecked would return
        let llm_body_only = "Result := x + y + 1";

        // Replace only the body directly (this simulates the case where
        // body extraction already happened or LLM returned just body)
        rewrite_feature_bodies(file.path(), &[(original_feature.name().to_owned(), llm_body_only)]).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Parse the modified content
        let (modified_class, _) = parser
            .class_and_tree_from_source(&modified_content)
            .expect("Should parse modified class");

        let modified_feature = modified_class
            .features()
            .iter()
            .find(|f| f.name() == "add")
            .expect("Should find add feature after modification");

        // Verify contracts are still present (unchanged)
        assert!(
            modified_feature.has_precondition(),
            "Precondition should be preserved when LLM returns just body"
        );
        assert!(
            modified_feature.has_postcondition(),
            "Postcondition should be preserved when LLM returns just body"
        );

        // Verify original precondition clauses are still there
        assert!(
            modified_content.contains("x >= 0"),
            "Original precondition clause 'x >= 0' should be preserved"
        );
        assert!(
            modified_content.contains("y >= 0"),
            "Original precondition clause 'y >= 0' should be preserved"
        );

        // Verify original postcondition clauses are still there
        assert!(
            modified_content.contains("Result >= x"),
            "Original postcondition clause 'Result >= x' should be preserved"
        );
        assert!(
            modified_content.contains("Result >= y"),
            "Original postcondition clause 'Result >= y' should be preserved"
        );

        // Verify the body was updated
        let modified_body = modified_feature
            .body_source_unchecked(modified_content.as_str())
            .expect("Should extract modified body");

        assert!(
            modified_body.contains("x + y + 1"),
            "Modified body should contain LLM's computation. Body was: {:?}",
            modified_body
        );
        // Note: "x + y" is part of "x + y + 1", so we check that the new computation is present
        let body_trimmed = modified_body.trim();
        assert!(
            body_trimmed.contains("+ 1") || body_trimmed.contains("x + y + 1"),
            "Modified body should contain '+ 1' or 'x + y + 1'. Body was: {:?}",
            modified_body
        );

        // Verify feature signature is unchanged
        assert!(
            modified_content.contains("add (x, y: INTEGER): INTEGER"),
            "Feature signature should be unchanged"
        );
    }

    #[tokio::test]
    async fn test_body_extraction_from_full_feature_preserves_contracts() {
        // This test directly tests the body extraction logic that happens
        // in fix_routine_in_place when LLM returns a full feature
        let mut parser = Parser::default();

        // Parse LLM's full feature response (with contracts)
        let (llm_class, _) = parser
            .class_and_tree_from_source(LLM_FULL_FEATURE_WITH_DIFFERENT_CONTRACTS)
            .expect("Fails to parse LLM-generated feature");

        let llm_feature = llm_class
            .features()
            .iter()
            .find(|f| f.name() == "add")
            .expect("Should find LLM-generated feature");

        // Extract body from full feature (simulating fix_routine_in_place behavior)
        let extracted_body = llm_feature
            .body_source_unchecked(LLM_FULL_FEATURE_WITH_DIFFERENT_CONTRACTS)
            .expect("Should extract body from full feature");

        // Verify extracted body contains only the computation, not contracts
        assert!(
            !extracted_body.contains("require"),
            "Extracted body should not contain 'require' keyword"
        );
        assert!(
            !extracted_body.contains("ensure"),
            "Extracted body should not contain 'ensure' keyword"
        );
        assert!(
            !extracted_body.contains("x > 0"),
            "Extracted body should not contain precondition clauses"
        );
        assert!(
            !extracted_body.contains("Result > x + y"),
            "Extracted body should not contain postcondition clauses"
        );
        assert!(
            extracted_body.contains("x + y + 1"),
            "Extracted body should contain the computation"
        );

        // Verify the extracted body is just the statement(s), not the full feature
        let trimmed_body = extracted_body.trim();
        assert!(
            trimmed_body == "Result := x + y + 1" || trimmed_body.contains("Result := x + y + 1"),
            "Extracted body should be just the computation statement"
        );
    }
}
