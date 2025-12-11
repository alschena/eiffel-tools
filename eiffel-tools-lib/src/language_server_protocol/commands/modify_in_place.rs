use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify;
use crate::parser;
use crate::workspace::Workspace;
use anyhow::anyhow;
use std::fs::OpenOptions;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use streaming_iterator::StreamingIterator;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tracing::info;
use tracing::warn;

// Query to find local_declarations within attribute_or_routine
static LOCAL_DECLARATIONS_QUERY: LazyLock<Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_eiffel::LANGUAGE.into(),
        r#"(attribute_or_routine
            (local_declarations) @local_declarations
        )"#,
    ).expect("Failed to create LOCAL_DECLARATIONS_QUERY")
});

// Query to find return_type node in feature_declaration
static RETURN_TYPE_QUERY: LazyLock<Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_eiffel::LANGUAGE.into(),
        r#"(feature_declaration
            type: (_) @return_type
        )"#,
    ).expect("Failed to create RETURN_TYPE_QUERY")
});

/// Find where the return type ends (if present) to determine signature end
fn find_return_type_end(
    feature: &Feature,
    source: &str,
) -> Option<Point> {
    // If feature has no return type, return None
    if feature.return_type().is_none() {
        return None;
    }
    
    // Parse the source to get the tree
    let (class, tree) = parser::Parser::default()
        .class_and_tree_from_source(source)
        .ok()?;
    
    // Find the matching feature
    let matching_feature = class
        .features()
        .iter()
        .find(|f| f.name() == feature.name())?;
    
    let feature_range = matching_feature.range();
    let source_bytes = source.as_bytes();
    let root_node = tree.root_node();
    
    // Get the capture index for return_type
    let return_type_index = RETURN_TYPE_QUERY
        .capture_index_for_name("return_type")?;
    
    // Create a query cursor to find return_type
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&RETURN_TYPE_QUERY, root_node, source_bytes);
    
    // Find the return_type that's within this feature's range
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index == return_type_index {
                let return_type_node = capture.node;
                let return_type_range: Range = return_type_node.range().into();
                
                // Check if this return_type is within the feature range
                if feature_range.contains(return_type_range.start) && feature_range.contains(return_type_range.end) {
                    return Some(return_type_range.end);
                }
            }
        }
    }
    
    None
}

/// Find the local declarations node range using tree-sitter query
fn find_local_clause_range(
    feature: &Feature,
    source: &str,
) -> Option<Range> {
    // Parse the source to get the tree
    let (class, tree) = parser::Parser::default()
        .class_and_tree_from_source(source)
        .ok()?;
    
    // Find the matching feature
    let matching_feature = class
        .features()
        .iter()
        .find(|f| f.name() == feature.name())?;
    
    let feature_range = matching_feature.range();
    let source_bytes = source.as_bytes();
    let root_node = tree.root_node();
    
    // Get the capture index for local_declarations
    let local_declarations_index = LOCAL_DECLARATIONS_QUERY
        .capture_index_for_name("local_declarations")?;
    
    // Create a query cursor to find local_declarations
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&LOCAL_DECLARATIONS_QUERY, root_node, source_bytes);
    
    // Find the local_declarations that's within this feature's range
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index == local_declarations_index {
                let local_node = capture.node;
                let local_range: Range = local_node.range().into();
                
                // Check if this local_declarations is within the feature range
                if feature_range.contains(local_range.start) && feature_range.contains(local_range.end) {
                    return Some(local_range);
                }
            }
        }
    }
    
    None
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

/// Rewrite feature bodies and local clauses, preserving contracts
pub async fn rewrite_feature_bodies_and_locals<'ft, B, I>(
    path: &Path,
    feature_bodies: I,
    llm_feature_sources: &[(&'ft FeatureName, &'ft str)],
) -> Option<String>
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    let (maybe_new_content, status) = tokio::fs::read(path)
        .await
        .inspect_err(|e| warn!("Fails to await reading {path:#?} before rewrite because {e:#?}"))
        .ok()
        .and_then(move |ref content| {
            str::from_utf8(content)
                .inspect_err(|e| warn!("Fails to convert file content to UFT-8 because {e:#?}"))
                .ok()
                .map(|initial_content| {
                    rewriting_feature_bodies_and_locals(initial_content, feature_bodies, llm_feature_sources)
                })
        })
        .unwrap_or((None, None));

    if let Some(new_content) = maybe_new_content {
        match tokio::fs::write(path, new_content).await {
            Ok(_) => status,
            Err(e) => {
                warn!("Fails to rewrite feature bodies and locals because {e:#?}");
                Some(format!("Failed to write file: {:#?}", e))
            }
        }
    } else {
        status
    }
}

/// Extract text from source within a given range (helper function)
fn extract_text_in_range(source: &str, range: &Range) -> String {
    let Range {
        start: Point { row: start_row, column: start_column },
        end: Point { row: end_row, column: end_column },
    } = *range;

    source
        .lines()
        .skip(start_row)
        .enumerate()
        .map_while(|(linenum, line)| match linenum {
            0 => Some(&line[start_column.min(line.len())..]),
            n if n < end_row - start_row => Some(line),
            n if n == end_row - start_row => Some(&line[..end_column.min(line.len())]),
            _ => None,
        })
        .fold(String::new(), |mut acc, line| {
            acc.push_str(line);
            acc.push('\n');
            acc
        })
}

/// Indent a block of code with the specified number of tabs per level
/// Removes existing indentation and applies consistent indentation using tabs
fn indent_code_block(code: &str, indent_level: usize) -> String {
    let indent = "\t".repeat(indent_level); // 1 tab per level
    let lines: Vec<&str> = code.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Indent a local clause preserving relative indentation
/// "local" keyword gets base_indent_level tabs, variable declarations get base_indent_level + 1 tabs
fn indent_local_clause(code: &str, base_indent_level: usize) -> String {
    let base_indent = "\t".repeat(base_indent_level);
    let var_indent = "\t".repeat(base_indent_level + 1);
    let lines: Vec<&str> = code.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    
    let result = lines
        .iter()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                // If this line is "local" keyword, use base indent (2 tabs)
                // Otherwise, it's a variable declaration - use base + 1 (3 tabs)
                if trimmed == "local" {
                    format!("{}{}", base_indent, trimmed)
                } else {
                    format!("{}{}", var_indent, trimmed)
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    result
}

fn rewriting_feature_bodies_and_locals<'ft, B, I>(
    initial_source: &str,
    feature_bodies: I,
    llm_feature_sources: &[(&'ft FeatureName, &'ft str)],
) -> (Option<String>, Option<String>)
where
    B: AsRef<str> + 'ft,
    I: IntoIterator<Item = &'ft (FeatureName, B)> + Copy,
{
    // #region agent log
    if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
        let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"ENTRY","location":"modify_in_place.rs:394","message":"rewriting_feature_bodies_and_locals called","data":{{"llm_feature_sources_count":{}}},"timestamp":{}}}"#, llm_feature_sources.len(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    parser::Parser::default()
        .class_and_tree_from_source(initial_source)
        .inspect_err(|e| warn!("Fails to parse file rewriting feature bodies and locals because {e:#?}"))
        .ok()
        .map(|(cl, _)| {
            // Build new file content by processing features in order using parser ranges
            let mut result = String::new();
            let mut last_pos = Point { row: 0, column: 0 };
            let mut status_messages = Vec::new();
            
            for feature in cl.features() {
                let feature_range = feature.range();
                
                // Extract text before this feature (from last position to feature start)
                // IMPORTANT: Extract up to column 0 of the feature line to avoid including the feature line itself
                if feature_range.start > last_pos {
                    let before_end = Point { row: feature_range.start.row, column: 0 };
                    if before_end > last_pos {
                        let before_range = Range { start: last_pos, end: before_end };
                        let before_text = extract_text_in_range(initial_source, &before_range);
                        // Trim trailing newlines to avoid double newlines
                        let before_text_trimmed = before_text.trim_end_matches('\n');
                        if !before_text_trimmed.is_empty() {
                            result.push_str(before_text_trimmed);
                            result.push('\n');
                        } else if !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                }
                
                // Check if this feature needs modification
                if let Some((_, new_body)) = matching_new_feature(feature.name(), feature_bodies) {
                    // Build modified feature content using parser ranges
                    
                    // 1. Extract feature signature (from feature start to return type end if present, or precondition start, or before local clause, or body start)
                    // Signature should end at return type if present, otherwise at precondition start, otherwise BEFORE local clause line, otherwise at body start
                    // IMPORTANT: Start from column 0 of the feature line to preserve indentation
                    let sig_start = Point { row: feature_range.start.row, column: 0 };
                    let existing_local_range = find_local_clause_range(feature, initial_source);
                    let sig_end = find_return_type_end(feature, initial_source)
                        .or_else(|| feature.point_start_preconditions())
                        .or_else(|| {
                            // If there's a local clause, end signature before it (at the end of previous line)
                            existing_local_range.as_ref().and_then(|lr| {
                                if lr.start.row > 0 {
                                    // Get the end of the line before the local clause
                                    let prev_line = initial_source.lines().nth(lr.start.row - 1)?;
                                    Some(Point { row: lr.start.row - 1, column: prev_line.len() })
                                } else {
                                    None
                                }
                            })
                        })
                        .or_else(|| feature.body_range().map(|br| br.start.clone()))
                        .unwrap_or_else(|| feature_range.end);
                    
                    let sig_range = Range { start: sig_start, end: sig_end };
                    let sig_text = extract_text_in_range(initial_source, &sig_range);
                    
                    // 2. Extract and preserve precondition
                    // IMPORTANT: Only extract if feature actually has a precondition
                    // IMPORTANT: Start from column 0 of the precondition line to preserve indentation
                    let pre_text = if feature.has_precondition() {
                        if let Some(pre_start) = feature.point_start_preconditions() {
                            if let Some(pre_end) = feature.point_end_preconditions() {
                                let pre_range_start = Point { row: pre_start.row, column: 0 };
                                let pre_range = Range { start: pre_range_start, end: pre_end };
                                extract_text_in_range(initial_source, &pre_range)
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    
                    // 2. Output signature and precondition
                    let sig_text_trimmed = sig_text.trim_end_matches('\n');
                    result.push_str(sig_text_trimmed);
                    if !sig_text_trimmed.ends_with('\n') {
                        result.push('\n');
                    }
                    if !pre_text.is_empty() {
                        let pre_text_trimmed = pre_text.trim_end_matches('\n');
                        result.push_str(pre_text_trimmed);
                        if !pre_text_trimmed.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    
                    // 3. Handle local clause: replace existing one if present, insert new one if LLM provided
                    // Note: existing_local_range was already computed above for sig_end calculation
                    
                    // Skip the existing local clause range (we'll replace it with LLM's version if provided)
                    // If there's an existing local clause, we need to skip it completely
                    // Don't extract any gap text - the LLM's local clause will be inserted with proper spacing
                    if existing_local_range.is_some() {
                        // Skip the old local clause - we'll insert the new one below
                        // No gap text extraction needed - the new local clause will handle spacing
                    } else {
                        // No existing local - extract any text between pre_end and body_start
                        // But don't include the "do" keyword line - we'll extract that separately
                        // IMPORTANT: If there's no local clause, there should be no gap text (just whitespace/newlines)
                        // We'll let the LLM's local clause insertion handle the spacing
                        // So we don't extract any gap text here when there's no existing local
                    }
                    
                    if let Some((_, llm_source)) = llm_feature_sources.iter().find(|(name, _)| **name == *feature.name()) {
                        // #region agent log
                        if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                            let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"A","location":"modify_in_place.rs:511","message":"Attempting to parse LLM source","data":{{"feature_name":"{}","llm_source_length":{}}},"timestamp":{}}}"#, feature.name(), llm_source.len(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        }
                        // #endregion
                        // Parse LLM source as a feature (not a class) since it's a feature-only snippet
                        // Try parsing as feature first (for feature-only snippets), fall back to class parsing
                        let mut parser = parser::Parser::default();
                        let llm_feature_result = parser.to_feature(llm_source)
                            .and_then(|parsed| match parsed {
                                parser::Parsed::Correct(feat) => Ok(feat),
                                parser::Parsed::HasErrorNodes(_, _) => {
                                    Err(anyhow!("Feature parsing had error nodes"))
                                }
                            });
                        
                        // #region agent log
                        if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                            let parse_success = llm_feature_result.is_ok();
                            let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"A","location":"modify_in_place.rs:523","message":"Feature parsing result","data":{{"feature_name":"{}","parse_success":{}}},"timestamp":{}}}"#, feature.name(), parse_success, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        }
                        // #endregion
                        
                        match llm_feature_result {
                            Ok(llm_feature) => {
                                // #region agent log
                                if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                                    let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"B","location":"modify_in_place.rs:525","message":"Successfully parsed as feature, extracting local clause","data":{{"feature_name":"{}"}},"timestamp":{}}}"#, feature.name(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                }
                                // #endregion
                                // Successfully parsed as feature - extract local clause
                                match llm_feature.local_clause_source_unchecked(*llm_source) {
                                    Ok(Some(llm_local)) => {
                                        // #region agent log
                                        if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                                            let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"B","location":"modify_in_place.rs:527","message":"Local clause extracted successfully","data":{{"feature_name":"{}","has_existing_local":{},"local_clause_length":{}}},"timestamp":{}}}"#, feature.name(), existing_local_range.is_some(), llm_local.len(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                        }
                                        // #endregion
                                        // If there's an existing local clause, replace its contents
                                        if existing_local_range.is_some() {
                                            // Simply replace the entire local_declarations block with LLM's version
                                            // Extract the full local clause from LLM (including "local" keyword)
                                            let llm_local_full = llm_local.trim();
                                            
                                            // Local clause should be at 2 tabs (standard Eiffel indentation)
                                            // Trim leading newlines to avoid double newlines
                                            let llm_local_trimmed = llm_local_full.trim_start_matches('\n');
                                            // For local clause: "local" keyword at 2 tabs, variable declarations at 3 tabs
                                            let indented_local = indent_local_clause(llm_local_trimmed, 2);
                                            
                                            result.push_str(&indented_local);
                                            if !indented_local.ends_with('\n') {
                                                result.push('\n');
                                            }
                                        } else {
                                            // No existing local clause - insert new one before body with proper indentation (2 tabs)
                                            // Trim leading newlines to avoid double newlines
                                            let llm_local_trimmed = llm_local.trim().trim_start_matches('\n');
                                            // For local clause: "local" keyword at 2 tabs, variable declarations at 3 tabs
                                            let indented_local = indent_local_clause(llm_local_trimmed, 2);
                                            result.push_str(&indented_local);
                                            if !indented_local.ends_with('\n') {
                                                result.push('\n');
                                            }
                                        }
                                        // #region agent log
                                        if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                                            let result_contains_local = result.contains("local");
                                            let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"C","location":"modify_in_place.rs:554","message":"Local clause inserted into result","data":{{"feature_name":"{}","result_contains_local":{},"result_length":{}}},"timestamp":{}}}"#, feature.name(), result_contains_local, result.len(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                        }
                                        // #endregion
                                    }
                                    Ok(None) => {
                                        // #region agent log
                                        if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                                            let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"B","location":"modify_in_place.rs:556","message":"LLM feature has no local clause","data":{{"feature_name":"{}"}},"timestamp":{}}}"#, feature.name(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                        }
                                        // #endregion
                                        // LLM feature doesn't have a local clause - this is fine, nothing to do
                                    }
                                    Err(e) => {
                                        let error_msg = format!(
                                            "Failed to extract local clause from LLM feature {}: {:#?}",
                                            feature.name(),
                                            e
                                        );
                                        warn!("{}", error_msg);
                                        status_messages.push(error_msg);
                                    }
                                }
                            }
                            Err(e) => {
                                // #region agent log
                                if let Ok(mut log_file) = OpenOptions::new().create(true).append(true).open("/home/ilgiz/uni/eiffel-tools/.cursor/debug.log") {
                                    let _ = writeln!(log_file, r#"{{"sessionId":"debug-session","runId":"pre-fix","hypothesisId":"A","location":"modify_in_place.rs:570","message":"Feature parsing failed, trying class parsing","data":{{"feature_name":"{}","error":"{:?}"}},"timestamp":{}}}"#, feature.name(), e, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                }
                                // #endregion
                                // Failed to parse as feature - try parsing as class (for full class snippets)
                                match parser::Parser::default().class_and_tree_from_source(llm_source) {
                                    Ok((llm_class, _)) => {
                                        if let Some(llm_feature) = llm_class.features().iter().find(|f| f.name() == feature.name()) {
                                            match llm_feature.local_clause_source_unchecked(*llm_source) {
                                                Ok(Some(llm_local)) => {
                                                    // If there's an existing local clause, replace its contents
                                                    if existing_local_range.is_some() {
                                                        let llm_local_full = llm_local.trim();
                                                        let llm_local_trimmed = llm_local_full.trim_start_matches('\n');
                                                        let indented_local = indent_local_clause(llm_local_trimmed, 2);
                                                        result.push_str(&indented_local);
                                                        if !indented_local.ends_with('\n') {
                                                            result.push('\n');
                                                        }
                                                    } else {
                                                        let llm_local_trimmed = llm_local.trim().trim_start_matches('\n');
                                                        let indented_local = indent_local_clause(llm_local_trimmed, 2);
                                                        result.push_str(&indented_local);
                                                        if !indented_local.ends_with('\n') {
                                                            result.push('\n');
                                                        }
                                                    }
                                                }
                                                Ok(None) => {
                                                    // LLM feature doesn't have a local clause - this is fine, nothing to do
                                                }
                                                Err(e) => {
                                                    let error_msg = format!(
                                                        "Failed to extract local clause from LLM feature {}: {:#?}",
                                                        feature.name(),
                                                        e
                                                    );
                                                    warn!("{}", error_msg);
                                                    status_messages.push(error_msg);
                                                }
                                            }
                                        } else {
                                            let error_msg = format!(
                                                "LLM source parsed as class but feature {} not found",
                                                feature.name()
                                            );
                                            warn!("{}", error_msg);
                                            status_messages.push(error_msg);
                                        }
                                    }
                                    Err(class_parse_err) => {
                                        let error_msg = format!(
                                            "Failed to parse LLM source for feature {} as feature or class. Feature parse error: {:#?}, Class parse error: {:#?}",
                                            feature.name(),
                                            e,
                                            class_parse_err
                                        );
                                        warn!("{}", error_msg);
                                        status_messages.push(error_msg);
                                    }
                                }
                            }
                        }
                    } else if existing_local_range.is_some() {
                        // LLM didn't provide a local clause, but there's an existing one - we should remove it
                        // (This case is handled by skipping it above, so nothing to do here)
                    }
                    
                    // 4. Replace body - extract "do" keyword with proper indentation, then insert new body
                    if let Some(body_range) = feature.body_range() {
                        // Extract "do" keyword and convert its indentation to tabs (2 tabs standard)
                        let do_line = initial_source
                            .lines()
                            .nth(body_range.start.row)
                            .unwrap_or("");
                        // Find "do" on the line and convert indentation to tabs
                        let _do_start = do_line.find("do").unwrap_or(0);
                        // Convert to 2 tabs (standard Eiffel indentation for "do")
                        let do_keyword = "\t\tdo".to_string();
                        result.push_str(&do_keyword);
                        
                        // Body content should always be at 3 tabs (standard Eiffel indentation)
                        let indented_body = indent_code_block(new_body.as_ref(), 3);
                        
                        // Add newline after "do" and then body (no extra blank line)
                        result.push('\n');
                        if !indented_body.is_empty() {
                            // Trim any leading newlines from indented_body to avoid double newlines
                            let body_trimmed = indented_body.trim_start_matches('\n');
                            result.push_str(body_trimmed);
                            if !body_trimmed.ends_with('\n') {
                                result.push('\n');
                            }
                        }
                    }
                    
                    // 5. Extract and preserve postcondition
                    // IMPORTANT: Find the "ensure" keyword line and extract from column 0 to preserve indentation
                    if let Some(post_start) = feature.point_start_postconditions() {
                        if let Some(post_end) = feature.point_end_postconditions() {
                            // Find the line containing "ensure" - it should be on the same row or the row before post_start
                            let ensure_row = if post_start.row > 0 {
                                let prev_line = initial_source.lines().nth(post_start.row - 1).unwrap_or("");
                                if prev_line.trim().starts_with("ensure") {
                                    post_start.row - 1
                                } else {
                                    post_start.row
                                }
                            } else {
                                post_start.row
                            };
                            let post_range_start = Point { row: ensure_row, column: 0 };
                            let post_range = Range { start: post_range_start, end: post_end };
                            result.push_str(&extract_text_in_range(initial_source, &post_range));
                        }
                    }
                    
                    // 6. Add "end" for the feature (2 tabs - feature level)
                    result.push_str("\t\tend\n");
                    
                    // Update last_pos to point after the feature's "end" keyword line
                    // Find the feature's "end" line in the original source
                    let feature_end_row = if let Some(post_end) = feature.point_end_postconditions().or_else(|| feature.body_range().map(|br| br.end.clone())) {
                        // The feature's "end" should be on the line after the postcondition/body
                        post_end.row + 1
                    } else {
                        // Fallback: use feature_range.end row (but this might point to class "end")
                        feature_range.end.row
                    };
                    // Make sure we don't go beyond the file
                    let lines_count = initial_source.lines().count();
                    if feature_end_row < lines_count {
                        let feature_end_line = initial_source.lines().nth(feature_end_row).unwrap_or("");
                        if feature_end_line.trim() == "end" {
                            // Point to the end of the feature's "end" line
                            last_pos = Point { row: feature_end_row, column: feature_end_line.len() };
                        } else {
                            // Fallback to feature_range.end
                            last_pos = feature_range.end;
                        }
                    } else {
                        last_pos = feature_range.end;
                    }
                } else {
                    // Feature doesn't need modification - copy it as-is using parser range
                    result.push_str(&extract_text_in_range(initial_source, &feature_range));
                    last_pos = feature_range.end;
                }
            }
            
            // Add remaining text after last feature
            // IMPORTANT: Start from column 0 of the line after the feature's "end" to preserve indentation
            let lines: Vec<&str> = initial_source.lines().collect();
            if !lines.is_empty() {
                // Find the line after the feature's "end" - skip the feature's "end" line itself
                let after_feature_end_row = if last_pos.row < lines.len() {
                    // Check if last_pos points to an "end" line - if so, start from the next line
                    let last_pos_line = lines.get(last_pos.row).map_or("", |v| v);
                    if last_pos_line.trim() == "end" {
                        last_pos.row + 1
                    } else {
                        last_pos.row
                    }
                } else {
                    last_pos.row
                };
                
                if after_feature_end_row < lines.len() {
                    let after_start = Point { row: after_feature_end_row, column: 0 };
                    let file_end = Point {
                        row: lines.len().saturating_sub(1),
                        column: lines.last().map(|l| l.len()).unwrap_or(0),
                    };
                    if file_end > after_start {
                        let after_range = Range { start: after_start, end: file_end };
                        let after_text = extract_text_in_range(initial_source, &after_range);
                        // Trim leading and trailing whitespace/newlines
                        let after_text_trimmed = after_text.trim();
                        if !after_text_trimmed.is_empty() {
                            // Ensure we have a newline after the trimmed text
                            result.push_str(after_text_trimmed);
                            if !after_text_trimmed.ends_with('\n') {
                                result.push('\n');
                            }
                        } else if !result.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                }
            }
            
            let status = if status_messages.is_empty() {
                None
            } else {
                Some(status_messages.join("; "))
            };
            (Some(result), status)
        })
        .unwrap_or((None, None))
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

    /// Normalize class text by:
    /// 1. Trimming each line (removing trailing whitespace)
    /// 2. Removing empty lines at start/end
    /// 3. Normalizing indentation (convert tabs to spaces)
    /// Note: Does NOT collapse repeating empty lines - preserves them as-is
    fn normalize_class_text(text: &str) -> String {
        let lines: Vec<String> = text
            .lines()
            .map(|line| {
                // Replace tabs with spaces for consistent indentation
                line.replace('\t', "    ").trim_end().to_string()
            })
            .collect();
        
        // Remove leading empty lines
        let mut trimmed: Vec<String> = lines
            .into_iter()
            .skip_while(|line| line.trim().is_empty())
            .collect();
        
        // Remove trailing empty lines
        while let Some(last) = trimmed.last() {
            if last.trim().is_empty() {
                trimmed.pop();
            } else {
                break;
            }
        }
        
        // Return normalized lines without collapsing consecutive empty lines
        trimmed.join("\n")
    }

    /// Assert that the actual class text matches the expected class text exactly
    /// Also verifies that the actual text can be parsed (syntax check)
    fn assert_class_text_matches(actual: &str, expected: &str, context: &str) {
        let normalized_actual = normalize_class_text(actual);
        let normalized_expected = normalize_class_text(expected);
        
        // First, verify the actual text can be parsed (syntax check)
        let mut parser = Parser::default();
        let parse_result = parser.class_and_tree_from_source(actual);
        assert!(
            parse_result.is_ok(),
            "{}: Generated class must be syntactically valid. Parse error: {:?}",
            context,
            parse_result.err()
        );
        
        // Assert proper indentation structure: body content should be at exactly 3 tabs
        let actual_lines: Vec<&str> = actual.lines().collect();
        for (i, line) in actual_lines.iter().enumerate() {
            if line.trim().starts_with("do") && i + 1 < actual_lines.len() {
                let next_line = actual_lines[i + 1];
                if !next_line.trim().is_empty() && !next_line.trim().starts_with("ensure") && !next_line.trim().starts_with("end") {
                    let body_indent = next_line.chars().take_while(|c| *c == '\t').count();
                    assert_eq!(
                        body_indent, 3,
                        "{}: Body content on line {} should be indented with exactly 3 tabs (got {} tabs): '{}'",
                        context, i + 2, body_indent, next_line
                    );
                }
            }
        }
        
        // Assert "do" keyword is at 2 tabs (or 8 spaces which normalize to 2 tabs)
        for (i, line) in actual_lines.iter().enumerate() {
            if line.trim() == "do" {
                let do_indent_tabs = line.chars().take_while(|c| *c == '\t').count();
                let do_indent_spaces = line.chars().take_while(|c| *c == ' ').count();
                // After normalization, 8 spaces = 2 tabs, so check normalized version
                let normalized_indent = if do_indent_tabs > 0 {
                    do_indent_tabs
                } else {
                    do_indent_spaces / 4  // 4 spaces per tab
                };
                assert_eq!(
                    normalized_indent, 2,
                    "{}: 'do' keyword on line {} should be indented with exactly 2 tabs (got {} tabs or {} spaces): '{}'",
                    context, i + 1, do_indent_tabs, do_indent_spaces, line
                );
            }
        }
        
        // Assert body content is at 3 tabs (or 12 spaces which normalize to 3 tabs)
        for (i, line) in actual_lines.iter().enumerate() {
            if i > 0 && actual_lines[i - 1].trim().starts_with("do") {
                if !line.trim().is_empty() && !line.trim().starts_with("ensure") && !line.trim().starts_with("end") {
                    let body_indent_tabs = line.chars().take_while(|c| *c == '\t').count();
                    let body_indent_spaces = line.chars().take_while(|c| *c == ' ').count();
                    let normalized_indent = if body_indent_tabs > 0 {
                        body_indent_tabs
                    } else {
                        body_indent_spaces / 4  // 4 spaces per tab
                    };
                    assert_eq!(
                        normalized_indent, 3,
                        "{}: Body content on line {} should be indented with exactly 3 tabs (got {} tabs or {} spaces): '{}'",
                        context, i + 1, body_indent_tabs, body_indent_spaces, line
                    );
                }
            }
        }
        
        // Assert "end" keyword is at 2 tabs (for feature end, not class end)
        for (i, line) in actual_lines.iter().enumerate() {
            if line.trim() == "end" && i > 0 {
                // Check if this is a feature end (not class end) by checking previous non-empty line
                let mut is_feature_end = false;
                for j in (0..i).rev() {
                    let prev_line = actual_lines[j];
                    if !prev_line.trim().is_empty() {
                        is_feature_end = prev_line.trim().starts_with("ensure") || 
                                        prev_line.trim().starts_with("do") ||
                                        prev_line.contains("Result") ||
                                        prev_line.contains(":=");
                        break;
                    }
                }
                if is_feature_end {
                    let end_indent_tabs = line.chars().take_while(|c| *c == '\t').count();
                    let end_indent_spaces = line.chars().take_while(|c| *c == ' ').count();
                    let normalized_indent = if end_indent_tabs > 0 {
                        end_indent_tabs
                    } else {
                        end_indent_spaces / 4  // 4 spaces per tab
                    };
                    assert_eq!(
                        normalized_indent, 2,
                        "{}: Feature 'end' keyword on line {} should be indented with exactly 2 tabs (got {} tabs or {} spaces): '{}'",
                        context, i + 1, end_indent_tabs, end_indent_spaces, line
                    );
                }
            }
        }
        
        // Then compare the normalized texts
        assert_eq!(
            normalized_actual,
            normalized_expected,
            "{}: Class text mismatch.\n\nExpected:\n{}\n\nActual:\n{}\n\n",
            context,
            normalized_expected,
            normalized_actual
        );
    }

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
        rewrite_feature_bodies_and_locals(file.path(), &[(feature.name().to_owned(), new_body)], &[]).await;

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

        // Verify the file matches our expected output exactly (with syntax check)
        assert_class_text_matches(
            &modified_content,
            FEATURE_WITH_NEW_BODY,
            "test_rewrite_feature_bodies_preserves_contracts"
        );
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

    const EXPECTED_FULL_FEATURE_BODY_REPLACED: &'static str = r#"
class TEST_CLASS
feature
    add (x, y: INTEGER): INTEGER
        require
            x >= 0
            y >= 0
        do
            Result := x + y + 1
        ensure
            Result >= x
            Result >= y
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

        // Replace only the body (using rewrite_feature_bodies_and_locals with empty locals)
        rewrite_feature_bodies_and_locals(file.path(), &[(original_feature.name().to_owned(), body_only)], &[]).await;

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

        // Verify the file matches our expected output exactly (with syntax check)
        assert_class_text_matches(
            &modified_content,
            EXPECTED_FULL_FEATURE_BODY_REPLACED,
            "test_llm_returns_full_feature_with_contracts_only_body_replaced"
        );
    }

    const EXPECTED_JUST_BODY_REPLACED: &'static str = r#"
class TEST_CLASS
feature
    add (x, y: INTEGER): INTEGER
        require
            x >= 0
            y >= 0
        do
            Result := x + y + 1
        ensure
            Result >= x
            Result >= y
        end
end
            "#;

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
        rewrite_feature_bodies_and_locals(file.path(), &[(original_feature.name().to_owned(), llm_body_only)], &[]).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Verify the file matches our expected output exactly (with syntax check)
        // (The assert_class_text_matches function already verifies syntax by parsing)
        assert_class_text_matches(
            &modified_content,
            EXPECTED_JUST_BODY_REPLACED,
            "test_llm_returns_just_body_only_body_replaced"
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

    const FEATURE_WITHOUT_LOCAL: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        do
            Result := a + b
        ensure
            Result >= a
            Result >= b
        end
end
            "#;

    // LLM returns only the feature (not wrapped in a class) - this is realistic
    const LLM_FEATURE_ONLY_WITH_LOCAL_AND_DIFFERENT_CONTRACTS: &'static str = r#"
    compute (a, b: INTEGER): INTEGER
        require
            a > 0
            b > 0
        local
            temp_a: INTEGER
            temp_b: INTEGER
        do
            temp_a := a
            temp_b := b
            Result := temp_a + temp_b
        ensure
            Result > a + b
        end
            "#;

    const EXPECTED_LOCAL_APPLIED_CONTRACTS_PRESERVED: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        local
            temp_a: INTEGER
            temp_b: INTEGER
        do
            temp_a := a
            temp_b := b
            Result := temp_a + temp_b
        ensure
            Result >= a
            Result >= b
        end
end
            "#;

    #[tokio::test]
    async fn test_llm_suggests_local_variables_and_contracts_only_local_applied() {
        // This test verifies that when LLM suggests adding local variables AND changing contracts,
        // the local variables ARE applied, but contracts are preserved (not applied).
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_local_and_contracts");

        // Write initial feature without local variables
        file.write_str(FEATURE_WITHOUT_LOCAL)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITHOUT_LOCAL)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Simulate LLM returning a feature-only snippet (not a full class) with local variables
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY_WITH_LOCAL_AND_DIFFERENT_CONTRACTS)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };
        let llm_full_source = LLM_FEATURE_ONLY_WITH_LOCAL_AND_DIFFERENT_CONTRACTS.to_string();

        // Simulate what fix_routine_in_place does: extract only the body
        let body_only = llm_feature
            .body_source_unchecked(llm_full_source.as_str())
            .expect("Should extract body from LLM-generated full feature");

        // Verify the extracted body contains the local variable assignments
        assert!(
            body_only.contains("temp_a := a"),
            "Extracted body should contain assignment to temp_a"
        );
        assert!(
            body_only.contains("temp_b := b"),
            "Extracted body should contain assignment to temp_b"
        );
        assert!(
            body_only.contains("temp_a + temp_b"),
            "Extracted body should contain computation using local variables"
        );
        
        // Verify the extracted body doesn't include contracts
        assert!(
            !body_only.contains("require"),
            "Extracted body should not contain require clause"
        );
        assert!(
            !body_only.contains("ensure"),
            "Extracted body should not contain ensure clause"
        );

        // Verify local clause can be extracted from LLM source using parser
        let local_clause = llm_feature
            .local_clause_source_unchecked(LLM_FEATURE_ONLY_WITH_LOCAL_AND_DIFFERENT_CONTRACTS)
            .expect("Should extract local clause from LLM-generated feature");
        assert!(
            local_clause.is_some(),
            "Should extract local clause from LLM-generated feature"
        );
        let local_clause_text = local_clause.unwrap();
        assert!(
            local_clause_text.contains("local"),
            "Extracted local clause should contain 'local' keyword"
        );
        assert!(
            local_clause_text.contains("temp_a: INTEGER"),
            "Extracted local clause should contain temp_a declaration"
        );
        assert!(
            local_clause_text.contains("temp_b: INTEGER"),
            "Extracted local clause should contain temp_b declaration"
        );

        // Apply both local clause and body (this is what rewrite_feature_bodies_and_locals does)
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), llm_full_source.as_str())],
        ).await;

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
            modified_content.contains("a >= 0"),
            "Original precondition clause 'a >= 0' should be preserved"
        );
        assert!(
            modified_content.contains("b >= 0"),
            "Original precondition clause 'b >= 0' should be preserved"
        );
        // Verify LLM's different precondition clauses are NOT present
        assert!(
            !modified_content.contains("a > 0"),
            "LLM's precondition clause 'a > 0' should NOT be present"
        );
        assert!(
            !modified_content.contains("b > 0"),
            "LLM's precondition clause 'b > 0' should NOT be present"
        );

        // Verify original postcondition clauses are still there
        assert!(
            modified_content.contains("Result >= a"),
            "Original postcondition clause 'Result >= a' should be preserved"
        );
        assert!(
            modified_content.contains("Result >= b"),
            "Original postcondition clause 'Result >= b' should be preserved"
        );
        // Verify LLM's different postcondition clause is NOT present
        assert!(
            !modified_content.contains("Result > a + b"),
            "LLM's postcondition clause 'Result > a + b' should NOT be present"
        );

        // Verify the 'local' clause WAS applied
        assert!(
            modified_content.contains("local"),
            "The 'local' clause SHOULD be present after applying LLM suggestion"
        );
        assert!(
            modified_content.contains("temp_a: INTEGER"),
            "Local variable declaration 'temp_a: INTEGER' should be present"
        );
        assert!(
            modified_content.contains("temp_b: INTEGER"),
            "Local variable declaration 'temp_b: INTEGER' should be present"
        );

        // Verify the body was updated with LLM's body (including local variable usage)
        let modified_body = modified_feature
            .body_source_unchecked(modified_content.as_str())
            .expect("Should extract modified body");

        assert!(
            modified_body.contains("temp_a := a"),
            "Modified body should contain assignment to temp_a"
        );
        assert!(
            modified_body.contains("temp_b := b"),
            "Modified body should contain assignment to temp_b"
        );
        assert!(
            modified_body.contains("temp_a + temp_b"),
            "Modified body should contain computation using local variables"
        );
    }

    const FEATURE_WITH_EXISTING_LOCAL: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        local
            old_var: INTEGER
        do
            old_var := a
            Result := old_var + b
        ensure
            Result >= a
            Result >= b
        end
end
            "#;

    // LLM returns only the feature (not wrapped in a class) - this is realistic
    const LLM_FEATURE_ONLY_WITH_DIFFERENT_LOCAL: &'static str = r#"
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        local
            new_var1: INTEGER
            new_var2: INTEGER
        do
            new_var1 := a
            new_var2 := b
            Result := new_var1 + new_var2
        ensure
            Result >= a
            Result >= b
        end
            "#;

    const EXPECTED_LOCAL_REPLACED: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        local
            new_var1: INTEGER
            new_var2: INTEGER
        do
            new_var1 := a
            new_var2 := b
            Result := new_var1 + new_var2
        ensure
            Result >= a
            Result >= b
        end
end
            "#;

    #[tokio::test]
    async fn test_replace_existing_local_clause_with_llm_suggestion() {
        // This test verifies that when a feature already has a local clause and LLM suggests a different one,
        // the old local clause is removed and replaced with LLM's suggestion.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_replace_local");

        // Write initial feature with existing local variables
        file.write_str(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Verify original feature has a local clause
        let original_local = original_feature
            .local_clause_source_unchecked(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Should extract local clause from original feature");
        assert!(
            original_local.is_some(),
            "Original feature should have a local clause"
        );
        let original_local_text = original_local.unwrap();
        assert!(
            original_local_text.contains("old_var: INTEGER"),
            "Original local clause should contain old_var"
        );

        // Simulate LLM returning a feature-only snippet (not a full class) with different local variables
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY_WITH_DIFFERENT_LOCAL)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };

        // Extract body from LLM feature
        let body_only = llm_feature
            .body_source_unchecked(LLM_FEATURE_ONLY_WITH_DIFFERENT_LOCAL)
            .expect("Should extract body from LLM-generated feature");

        // Apply both local clause and body
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), LLM_FEATURE_ONLY_WITH_DIFFERENT_LOCAL)],
        ).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Verify OLD local clause is NOT present
        assert!(
            !modified_content.contains("old_var: INTEGER"),
            "Old local variable 'old_var' should be removed"
        );

        // Verify NEW local clause IS present
        assert!(
            modified_content.contains("local"),
            "The 'local' clause should be present"
        );
        assert!(
            modified_content.contains("new_var1: INTEGER"),
            "New local variable 'new_var1' should be present"
        );
        assert!(
            modified_content.contains("new_var2: INTEGER"),
            "New local variable 'new_var2' should be present"
        );

        // Verify there's only ONE local block (not two)
        let local_count = modified_content.matches("\tlocal").count();
        assert_eq!(
            local_count, 1,
            "There should be exactly one local block, found {}",
            local_count
        );

        // Verify the body uses the new local variables
        let (modified_class, _) = parser
            .class_and_tree_from_source(&modified_content)
            .expect("Should parse modified class");
        let modified_feature = modified_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature after modification");
        
        let modified_body = modified_feature
            .body_source_unchecked(modified_content.as_str())
            .expect("Should extract modified body");

        assert!(
            modified_body.contains("new_var1 := a"),
            "Modified body should use new_var1"
        );
        assert!(
            modified_body.contains("new_var2 := b"),
            "Modified body should use new_var2"
        );
        assert!(
            !modified_body.contains("old_var"),
            "Modified body should not use old_var"
        );
    }

    // LLM returns only the feature (not wrapped in a class) - this is realistic
    const LLM_FEATURE_ONLY_WITHOUT_LOCAL: &'static str = r#"
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        do
            Result := a + b
        ensure
            Result >= a
            Result >= b
        end
            "#;

    const EXPECTED_LOCAL_REMOVED: &'static str = r#"
class TEST_CLASS
feature

compute (a, b: INTEGER): INTEGER
require
            a >= 0
            b >= 0

do

                Result := a + b
ensure
            Result >= a
            Result >= b
    end

end
            "#;

    #[tokio::test]
    async fn test_remove_local_clause_when_llm_suggests_no_local() {
        // This test verifies that when a feature has a local clause and LLM suggests removing it
        // (by providing a feature without local), the local clause is removed from the resulting feature.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_remove_local");

        // Write initial feature with existing local variables
        file.write_str(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Verify original feature has a local clause
        let original_local = original_feature
            .local_clause_source_unchecked(FEATURE_WITH_EXISTING_LOCAL)
            .expect("Should extract local clause from original feature");
        assert!(
            original_local.is_some(),
            "Original feature should have a local clause"
        );

        // Simulate LLM returning a feature-only snippet (not a full class) WITHOUT local variables
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY_WITHOUT_LOCAL)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };

        // Verify LLM feature has NO local clause
        let llm_local = llm_feature
            .local_clause_source_unchecked(LLM_FEATURE_ONLY_WITHOUT_LOCAL)
            .expect("Should check for local clause in LLM feature");
        assert!(
            llm_local.is_none(),
            "LLM feature should NOT have a local clause"
        );

        // Extract body from LLM feature
        let body_only = llm_feature
            .body_source_unchecked(LLM_FEATURE_ONLY_WITHOUT_LOCAL)
            .expect("Should extract body from LLM-generated feature");

        // Apply body (and no local clause since LLM didn't provide one)
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), LLM_FEATURE_ONLY_WITHOUT_LOCAL)],
        ).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Verify local clause is NOT present
        assert!(
            !modified_content.contains("local"),
            "The 'local' clause should be removed when LLM suggests no local"
        );
        assert!(
            !modified_content.contains("old_var: INTEGER"),
            "Old local variable should be removed"
        );

        // Verify there's NO local block
        let local_count = modified_content.matches("\tlocal").count();
        assert_eq!(
            local_count, 0,
            "There should be no local block, found {}",
            local_count
        );

        // Verify the file matches our expected output exactly (with syntax check)
        assert_class_text_matches(
            &modified_content,
            EXPECTED_LOCAL_REMOVED,
            "test_remove_local_clause_when_llm_suggests_no_local"
        );
    }

    const FEATURE_NO_PRECONDITION_WITH_LOCAL: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        local
            temp: INTEGER
        do
            temp := a
            Result := temp + b
        end
end
            "#;

    // LLM returns only the feature (not wrapped in a class) - this is realistic
    const LLM_FEATURE_ONLY_NO_PRECONDITION_NEW_LOCAL: &'static str = r#"
    compute (a, b: INTEGER): INTEGER
        local
            new_temp: INTEGER
        do
            new_temp := a + b
            Result := new_temp
        end
            "#;

    const EXPECTED_NO_PRECONDITION_LOCAL_REPLACED: &'static str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        local
            new_temp: INTEGER
        do
            new_temp := a + b
            Result := new_temp
        end
end
            "#;

    #[tokio::test]
    async fn test_replace_local_clause_when_no_precondition() {
        // This test verifies that local clause replacement works correctly
        // when there's no precondition (edge case for signature extraction)
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_replace_local_no_precondition");

        // Write initial feature with local but no precondition
        file.write_str(FEATURE_NO_PRECONDITION_WITH_LOCAL)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_NO_PRECONDITION_WITH_LOCAL)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Verify original feature has a local clause
        let original_local = original_feature
            .local_clause_source_unchecked(FEATURE_NO_PRECONDITION_WITH_LOCAL)
            .expect("Should extract local clause from original feature");
        assert!(
            original_local.is_some(),
            "Original feature should have a local clause"
        );

        // Simulate LLM returning a feature-only snippet (not a full class) with different local variable
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY_NO_PRECONDITION_NEW_LOCAL)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };

        // Extract body from LLM feature
        let body_only = llm_feature
            .body_source_unchecked(LLM_FEATURE_ONLY_NO_PRECONDITION_NEW_LOCAL)
            .expect("Should extract body from LLM-generated feature");

        // Apply both local clause and body
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), LLM_FEATURE_ONLY_NO_PRECONDITION_NEW_LOCAL)],
        ).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Verify OLD local clause is NOT present
        assert!(
            !modified_content.contains("original: INTEGER"),
            "Old local variable 'original' should be removed"
        );

        // Verify NEW local clause IS present
        assert!(
            modified_content.contains("local"),
            "The 'local' clause should be present"
        );
        assert!(
            modified_content.contains("new_temp: INTEGER"),
            "New local variable 'new_temp' should be present"
        );

        // Verify there's only ONE local block
        let local_count = modified_content.matches("\tlocal").count();
        assert_eq!(
            local_count, 1,
            "There should be exactly one local block, found {}",
            local_count
        );

        // Verify the file matches our expected output exactly (with syntax check)
        assert_class_text_matches(
            &modified_content,
            EXPECTED_NO_PRECONDITION_LOCAL_REPLACED,
            "test_replace_local_clause_when_no_precondition"
        );
    }

    #[test]
    fn test_signature_ends_at_return_type() {
        // Test that signature extraction correctly ends at return type
        let source = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a > 0
        local
            temp: INTEGER
        do
            Result := a + b
        ensure
            Result > 0
        end
end
            "#;

        let mut parser = Parser::default();
        let (class, _) = parser
            .class_and_tree_from_source(source)
            .expect("Should parse class");
        let feature = class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Test find_return_type_end
        let return_type_end = find_return_type_end(feature, source);
        assert!(
            return_type_end.is_some(),
            "Should find return type end for feature with return type"
        );

        if let Some(end_point) = return_type_end {
            // Verify that the return type end point is within the feature range
            let feature_range = feature.range();
            assert!(
                end_point.row >= feature_range.start.row && end_point.row <= feature_range.end.row,
                "Return type end should be within feature range"
            );
            
            // The return type should end on the same line as the signature
            // (line with "compute (a, b: INTEGER): INTEGER")
            let lines: Vec<&str> = source.lines().collect();
            if end_point.row < lines.len() {
                let signature_line = lines[end_point.row];
                // The signature line should contain the return type
                assert!(
                    signature_line.contains("INTEGER") && signature_line.contains("compute"),
                    "Return type end should be on the signature line"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_llm_suggests_adding_local_but_not_applied() {
        // This test demonstrates the bug: when LLM suggests adding a local variable
        // to a feature that doesn't have one, the local clause is not applied.
        // This matches the scenario from the JSON data where llm_message includes
        // "local\n        l_k: INTEGER\n" but after_code doesn't have it.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_add_local_bug");

        const FEATURE_WITHOUT_LOCAL: &'static str = r#"
class MAPLE_RECURSIVE_SUM_N_4
feature
    sum (a_n: INTEGER): INTEGER
        require
            argument_non_negative: a_n >= 0
        do
            if a_n = 0 then
                Result := 0
            else
                l_k := sum (a_n - 1)
                Result := l_k + a_n
            end
        ensure
            correct_result: 2 * Result = a_n * (a_n + 1)
        end
end
            "#;


        // Write initial feature without local
        file.write_str(FEATURE_WITHOUT_LOCAL)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(FEATURE_WITHOUT_LOCAL)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "sum")
            .expect("Should find sum feature");

        // Verify original feature has NO local clause
        let original_local = original_feature
            .local_clause_source_unchecked(FEATURE_WITHOUT_LOCAL)
            .expect("Should check for local clause");
        assert!(
            original_local.is_none(),
            "Original feature should NOT have a local clause"
        );

        // Simulate LLM returning a feature-only snippet (not a full class) with local variable
        // This is what the LLM actually returns - just the feature, not wrapped in a class
        const LLM_FEATURE_ONLY_WITH_LOCAL: &'static str = r#"
    sum (a_n: INTEGER): INTEGER
        require
            argument_non_negative: a_n >= 0
        local
            l_k: INTEGER
        do
            if a_n = 0 then
                Result := 0
            else
                l_k := sum (a_n - 1)
                Result := l_k + a_n
            end
        ensure
            correct_result: 2 * Result = a_n * (a_n + 1)
        end
            "#;

        // Parse the feature-only snippet using to_feature (not class_and_tree_from_source)
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY_WITH_LOCAL)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };
        let llm_full_source = LLM_FEATURE_ONLY_WITH_LOCAL.to_string();

        // Verify LLM feature HAS a local clause
        let llm_local = llm_feature
            .local_clause_source_unchecked(llm_full_source.as_str())
            .expect("Should extract local clause from LLM feature");
        assert!(
            llm_local.is_some(),
            "LLM feature should have a local clause"
        );
        let llm_local_text = llm_local.unwrap();
        assert!(
            llm_local_text.contains("l_k: INTEGER"),
            "LLM local clause should contain l_k"
        );

        // Extract body from LLM feature
        let body_only = llm_feature
            .body_source_unchecked(llm_full_source.as_str())
            .expect("Should extract body from LLM-generated full feature");

        // Apply both local clause and body (this is what rewrite_feature_bodies_and_locals does)
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), llm_full_source.as_str())],
        ).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // THIS IS THE BUG: The local clause should be present but it's not
        // Verify NEW local clause IS present (this will fail, demonstrating the bug)
        assert!(
            modified_content.contains("local"),
            "BUG DEMONSTRATED: The 'local' clause should be present but it's missing. Modified content:\n{}",
            modified_content
        );
        assert!(
            modified_content.contains("l_k: INTEGER"),
            "BUG DEMONSTRATED: Local variable 'l_k' should be present but it's missing. Modified content:\n{}",
            modified_content
        );

        // Verify the body uses the local variable
        assert!(
            modified_content.contains("l_k := sum (a_n - 1)"),
            "Body should use the local variable l_k"
        );
    }

    #[tokio::test]
    async fn test_add_local_block_feature_only_snippet() {
        // This test specifically verifies that when LLM returns a feature-only snippet
        // (not a full class) with a local block, it is correctly extracted and applied.
        let tmp_dir = TempDir::new().expect(stringify!(
            "Fails to create temporary directory for testing. {} {}:{}",
            file!(),
            line!(),
            column!()
        ));
        let file = tmp_dir.child("test_add_local_feature_only");

        const ORIGINAL_FEATURE: &'static str = r#"
class TEST_CLASS
feature
    compute (x: INTEGER): INTEGER
        require
            x >= 0
        do
            Result := x * 2
        ensure
            Result >= x
        end
end
            "#;

        // LLM returns only the feature (not wrapped in a class) - this is realistic
        const LLM_FEATURE_ONLY: &'static str = r#"
    compute (x: INTEGER): INTEGER
        require
            x >= 0
        local
            temp: INTEGER
        do
            temp := x
            Result := temp * 2
        ensure
            Result >= x
        end
            "#;

        // Write initial feature without local
        file.write_str(ORIGINAL_FEATURE)
            .expect("Fails to write initial feature");

        // Parse the original feature
        let mut parser = Parser::default();
        let (original_class, tree) = parser
            .class_and_tree_from_source(ORIGINAL_FEATURE)
            .expect("Fails to parse original class");
        let mut ws = Workspace::new();
        ws.add_file((original_class.clone(), file.to_path_buf(), tree));

        let original_feature = original_class
            .features()
            .iter()
            .find(|f| f.name() == "compute")
            .expect("Should find compute feature");

        // Verify original feature has NO local clause
        let original_local = original_feature
            .local_clause_source_unchecked(ORIGINAL_FEATURE)
            .expect("Should check for local clause");
        assert!(
            original_local.is_none(),
            "Original feature should NOT have a local clause"
        );

        // Parse LLM feature-only snippet using to_feature (not class_and_tree_from_source)
        let llm_feature = match parser
            .to_feature(LLM_FEATURE_ONLY)
            .expect("Should parse feature-only snippet")
        {
            parser::Parsed::Correct(feat) => feat,
            parser::Parsed::HasErrorNodes(_, _) => panic!("Feature should parse correctly"),
        };

        // Verify LLM feature HAS a local clause
        let llm_local = llm_feature
            .local_clause_source_unchecked(LLM_FEATURE_ONLY)
            .expect("Should extract local clause from LLM feature");
        assert!(
            llm_local.is_some(),
            "LLM feature should have a local clause"
        );
        let llm_local_text = llm_local.unwrap();
        assert!(
            llm_local_text.contains("temp: INTEGER"),
            "LLM local clause should contain temp"
        );

        // Extract body from LLM feature
        let body_only = llm_feature
            .body_source_unchecked(LLM_FEATURE_ONLY)
            .expect("Should extract body from LLM-generated feature");

        // Apply both local clause and body (this is what rewrite_feature_bodies_and_locals does)
        rewrite_feature_bodies_and_locals(
            file.path(),
            &[(original_feature.name().to_owned(), body_only)],
            &[(original_feature.name(), LLM_FEATURE_ONLY)],
        ).await;

        // Read the modified file
        let modified_content = tokio::fs::read_to_string(file.path())
            .await
            .expect("Should read modified file");

        // Verify NEW local clause IS present
        assert!(
            modified_content.contains("local"),
            "The 'local' clause should be present. Modified content:\n{}",
            modified_content
        );
        assert!(
            modified_content.contains("temp: INTEGER"),
            "Local variable 'temp' should be present. Modified content:\n{}",
            modified_content
        );

        // Verify the body uses the local variable
        assert!(
            modified_content.contains("temp := x"),
            "Body should use the local variable temp"
        );
        assert!(
            modified_content.contains("Result := temp * 2"),
            "Body should use temp in computation"
        );

        // Verify there's only ONE local block
        let local_count = modified_content.matches("\tlocal").count();
        assert_eq!(
            local_count, 1,
            "There should be exactly one local block, found {}",
            local_count
        );

        // Verify contracts are preserved
        assert!(
            modified_content.contains("require"),
            "Precondition should be preserved"
        );
        assert!(
            modified_content.contains("ensure"),
            "Postcondition should be preserved"
        );
    }
}
