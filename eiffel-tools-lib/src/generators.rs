use crate::code_entities::prelude::*;
use crate::parser::Parser;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use contract::RoutineSpecification;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

mod prompt;

mod constructor_api;

#[derive(Debug, Default)]
pub struct Generators {
    llms: Vec<Arc<constructor_api::Llm>>,
}

impl Generators {
    pub async fn add_new(&mut self) {
        let Ok(llm) = constructor_api::Llm::try_new().await else {
            info!("fail to create LLM via constructor API");
            return;
        };
        self.llms.push(Arc::new(llm));
    }
}

mod feature_focused {
    use super::*;

    impl Generators {
        pub async fn more_routine_specifications(
            &self,
            feature: &Feature,
            workspace: &Workspace,
            path: &Path,
        ) -> Result<Vec<RoutineSpecification>> {
            let prompt =
                prompt::FeaturePrompt::try_new_for_feature_specification(workspace, path, feature)
                    .await?;

            // Generate feature with specifications
            let completion_parameters = constructor_api::CompletionParameters {
                messages: prompt.into(),
                n: Some(50),
                ..Default::default()
            };

            info!(target:"llm", "{completion_parameters:#?}");

            let mut tasks = tokio::task::JoinSet::new();
            for llm in self.llms.iter().cloned() {
                let completion_parameters = completion_parameters.clone();
                tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
            }
            let completion_response = tasks.join_all().await;

            let completion_response_processed = completion_response
            .iter()
            .filter_map(|rs| match rs {
                Err(e) => {
                    warn!(target:"llm", "An LLM request has returned the error: {e:#?}");
                    None
                }
                Ok(reply) => Some(reply),
            })
            .flat_map(|reply| {
                reply
                    .extract_multiline_code()
                    .into_iter()
                    .filter_map(|candidate| {
                        info!(target:"llm", "candidate:\t{candidate}");
                        let mut parser = Parser::new();
                        parser.feature_from_source(&candidate).map_or_else(
                            |e| {
                                warn!(target:"llm", "fail to parse generated output with error: {e:#?}");
                                None
                            },
                            |ft| Some(ft.routine_specification()),
                        )
                    })
            })
            .collect();

            info!("completions:\t{completion_response_processed:#?}");

            Ok(completion_response_processed)
        }

        pub async fn fix_body(
            &self,
            path: &Path,
            feature: &Feature,
            error_message: String,
        ) -> Result<Option<String>> {
            let prompt =
                prompt::FeaturePrompt::try_new_for_feature_fixes(path, feature, error_message)
                    .await
                    .with_context(|| format!("fails to make prompt to fix routine"))?
                    .into();

            // Generate feature with specifications
            let completion_parameters = constructor_api::CompletionParameters {
                messages: prompt,
                n: Some(5),
                ..Default::default()
            };

            info!(target: "llm", "completion parameters: {:#?}", completion_parameters);

            let mut tasks = tokio::task::JoinSet::new();
            for llm in self.llms.iter().cloned() {
                let completion_parameters = completion_parameters.clone();
                tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
            }
            let completion_response = tasks.join_all().await;

            let completion_response_processed: Option<String> = completion_response
                .iter()
                .filter_map(|maybe_response| {
                    maybe_response.as_ref()
                        .inspect_err(|e| {
                            warn!(
                                target = "llm",
                                "An LLM processing the feature fix has returned the error: {:#?}",
                                e
                            )
                        })
                        .ok()
                })
                .flat_map(|response| response.extract_multiline_code().into_iter())
                .inspect(|candidate| {
                    info!(target: "llm", "candidate:\t{candidate}");
                })
                .filter_map(|candidate| {
                    let mut parser = Parser::new();

                    let ft = parser
                        .feature_from_source(&candidate)
                        .inspect_err(|e| {
                            info!(target: "llm", "fails to parse LLM generated feature with error: {e:#?}");
                        })
                        .ok()?;

                    ft.body_source_unchecked(candidate).inspect_err(|e| info!(target: "llm", "fails to extract body of candidate feature with error: {:#?}", e)).ok()
                })
                .inspect(|filtered_candidate| {
                    info!(target: "llm", "candidate of correct body:\t{:#?}", filtered_candidate);
                })
                .next();

            if completion_response_processed.is_none() {
                info!(target:"llm", "llm proposes no candidate.");
            }

            Ok(completion_response_processed)
        }
    }
}

mod class_wide {
    use super::*;

    impl Generators {
        pub async fn class_wide_specifications(
            &self,
            workspace: &Workspace,
            path: &Path,
        ) -> Result<String> {
            let class = workspace.class(path).unwrap();

            let prompt = prompt::ClassPrompt::try_new_for_model_based_contracts(workspace, class)
                .await
                .unwrap();

            // Generate feature with specifications
            let completion_parameters = constructor_api::CompletionParameters {
                messages: prompt.into(),
                n: Some(5),
                ..Default::default()
            };

            info!(target:"llm", "{completion_parameters:#?}");

            let mut tasks = tokio::task::JoinSet::new();
            for llm in self.llms.iter().cloned() {
                let completion_parameters = completion_parameters.clone();
                tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
            }
            let completion_response = tasks.join_all().await;

            let completion_response_processed = completion_response
                .iter()
                .filter_map(|rs| match rs {
                    Err(e) => {
                        warn!(target:"llm", "An LLM request has returned the error: {e:#?}");
                        None
                    }
                    Ok(reply) => Some(reply),
                })
                .flat_map(|reply| {
                    reply.extract_multiline_code().into_iter()
                    // TODO generate routine specs for each feature
                })
                .collect();

            println!("completions:\t{completion_response_processed:#?}");
            // info!("completions:\t{completion_response_processed:#?}");

            Ok(completion_response_processed)
        }

        /// List of LLM generated feature candidates.
        /// Each tuple in the list can be described by this pattern naming: (feature_name: String, llm_candidate_for_feature: Option<String>)
        pub async fn class_wide_fixes(
            &self,
            workspace: &Workspace,
            path: &Path,
            error_message: String,
        ) -> Vec<(String, String)> {
            let class = workspace
                .class(path)
                .unwrap_or_else(|| panic!("fails to find class at {:#?}", path));

            let prompt =
                prompt::ClassPrompt::try_new_for_feature_fixes(workspace, class, error_message)
                    .await
                    .expect("fails to produce prompt for class-wide fixes.");

            let completion_parameters = constructor_api::CompletionParameters {
                messages: prompt.into(),
                n: Some(5),
                ..Default::default()
            };

            println!(
                "completion parameters for class-wide fixes: {:#?}",
                completion_parameters
            );

            let mut tasks = tokio::task::JoinSet::new();
            for llm in self.llms.iter().cloned() {
                let completion_parameters = completion_parameters.clone();
                tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
            }
            let completion_response = tasks.join_all().await;

            let first_parsable_response = completion_response
                .into_iter()
                .filter_map(|response| {
                    response
                        .inspect_err(|e| eprintln!("One llm returns  {:#?}", e))
                        .ok()
                })
                .flat_map(|response| response.extract_multiline_code())
                .inspect(|candidate| println!("unfiltered candidate class: {:#?}", candidate))
                .filter_map(|candidate| {
                    let mut parser = Parser::new();
                    parser
                        .class_and_tree_from_source(&candidate)
                        .inspect_err(|e| {
                            eprintln!(
                                "fails to parse generated class:\n{candidate}\nbecause {e:#?}"
                            )
                        })
                        .ok()
                        .map(|(class, _)| (class, candidate))
                })
                .map(|(class, candidate_class_text)| {
                    class
                        .features()
                        .into_iter()
                        .map(|ft| (ft.name(), ft.range()))
                        .inspect(|(name, range)| {
                            eprintln!("name: {name:#?}");
                            eprintln!("range: {range:#?}");
                            eprintln!(
                                "extraction: {:#?}",
                                extract_text_within_range(&candidate_class_text, range)
                            );
                        })
                        .map(|(name, range)| {
                            (
                                name.to_string(),
                                extract_text_within_range(&candidate_class_text, range)
                                    .trim_end()
                                    .to_string(),
                            )
                        })
                        .inspect(|(name, possible_content)| {
                            eprintln!("candidates: name: {name}, {possible_content:#?}")
                        })
                        .collect::<Vec<_>>()
                })
                .next();

            first_parsable_response.unwrap_or_default()
        }
    }

    fn extract_text_within_range(candidate: &str, range: &Range) -> String {
        let &Range {
            start:
                Point {
                    row: start_row,
                    column: start_column,
                },
            end:
                Point {
                    row: end_row,
                    column: end_column,
                },
        } = range;

        candidate .lines()
            .skip(start_row)
            .enumerate()
            .inspect(|(linenum, line_candidate)| {
                eprintln!(
                    "linenum: {linenum}\nline_candidate: {line_candidate}"
                );
                eprintln!(
                    "start row: {start_row}\nstart column: {start_column}\nend row: {end_row}\nend column: {end_column}"
                );
            })
            .map_while(|(linenum, line)| match linenum {
                0 => Some(&line[start_column..]),
                n if n < end_row - start_row => Some(line),
                n if n == end_row - start_row => Some(&line[..end_column]),
                _ => None,
            })
            .fold(String::new(), |acc, line| format!("{acc}{line}\n"))
    }
}

#[cfg(test)]
impl Generators {
    pub fn mock() -> Self {
        Generators { llms: Vec::new() }
    }
}
