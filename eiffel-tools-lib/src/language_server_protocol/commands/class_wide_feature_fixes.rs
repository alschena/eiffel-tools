use crate::code_entities::prelude::*;
use crate::eiffelstudio_cli::VerificationResult;
use crate::eiffelstudio_cli::verify_class;
use crate::generators::Generators;
use crate::parser::Parser;
use crate::workspace::Workspace;
use std::path::Path;

pub async fn fix_class_in_place(
    generators: &Generators,
    workspace: &mut Workspace,
    class_name: &ClassName,
) {
    println!("fix class in place {class_name}");

    let max_number_of_tries = 10;
    let mut number_of_tries = 0;
    loop {
        match verify_class(class_name).await {
            Some(VerificationResult::Success) => {
                println!("{} successfully verified.", class_name);
                break;
            }
            Some(VerificationResult::Failure(error_message))
                if number_of_tries < max_number_of_tries =>
            {
                println!("The class did not verify at try #{number_of_tries}");

                number_of_tries += 1;

                let path = workspace.path(class_name).to_path_buf();

                workspace.reload(path.clone()).await;

                let class = workspace
                    .class(&path)
                    .expect("fails to get reloaded class.");

                let feature_candidates = generators
                    .class_wide_fixes(workspace, class, error_message)
                    .await;

                rewrite_features(workspace.path(class_name), feature_candidates).await;
            }
            Some(VerificationResult::Failure(error_message)) => {
                eprintln!(
                    "after {max_number_of_tries} tries, {class_name} still fails to verify. The last error message follows:\n{error_message}"
                );
                break;
            }
            None => {
                eprintln!(
                    "AutoProof fails either for the logic in the function `verify_class` or because of an internal processing error of AutoProof."
                );
                break;
            }
        }
    }
}

async fn rewrite_features(path: &Path, features: Vec<(String, String)>) {
    let Ok(current_file) = tokio::fs::read(path)
        .await
        .inspect_err(|e| eprintln!("fails to read {path:#?} with {e:#?}"))
    else {
        return;
    };

    let Ok(current_file) = String::from_utf8(current_file)
        .inspect_err(|e| eprintln!("fails to convert current file to UTF-8 string with {e:#?}"))
    else {
        return;
    };

    let mut parser = Parser::new();
    let Ok((class, _)) = parser
        .class_and_tree_from_source(&current_file)
        .inspect_err(|e| eprintln!("fails to parse current file {path:#?} with {e:#?}"))
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
        features
            .iter()
            .inspect(|(name, content)| {
                eprintln!("name new feature: {name}\ncontent new feature: {content}")
            })
            .find(|(name_of_new, _)| feature_name == name_of_new)
    };

    let on_starting_feature = |acc: &mut String, linenum, line: &str| {
        starting_current_feature(linenum).and_then(|(name, range)| {
            matching_new_feature(name).map(|(_, content)| {
                if range.end.row != range.start.row {
                    format!("{}{}{}\n", acc, &line[..range.start.column], content)
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
            .inspect_err(|e| {
                eprintln!("fails to await for rewriting file at {path:#?} with {e:#?}")
            })
            .ok();
    }
}
