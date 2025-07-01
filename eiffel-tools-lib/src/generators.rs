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
            warn!("fail to create LLM via constructor API");
            return;
        };
        self.llms.push(Arc::new(llm));
    }

    async fn complete(
        &self,
        parameters: constructor_api::CompletionParameters,
    ) -> impl IntoIterator<Item = constructor_api::CompletionResponse> {
        info!(target:"llm", "{parameters:#?}");

        let mut tasks = tokio::task::JoinSet::new();
        for llm in self.llms.iter().cloned() {
            let completion_parameters = parameters.clone();
            tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
        }
        let completion_response = tasks.join_all().await;

        completion_response.into_iter().filter_map(|rs| {
            rs.inspect_err(|e| warn!(target:"llm", "An LLM request has returned the error: {e:#?}"))
                .ok()
        })
    }
}

mod feature_focused {
    use super::*;
    use crate::parser::Parsed;

    fn filter_unparsable(candidate: String) -> Option<(Feature, String)> {
        match Parser::new().to_feature(&candidate) {
            Err(e) => {
                info!(target: "llm", "Fails to parse LLM generated feature with error: {e:#?}");
                None
            }
            Ok(Parsed::Correct(val)) => {
                info!(target: "llm", "Parsable LLM candidate:\t{:#?}", candidate);
                Some((val, candidate))
            }
            Ok(Parsed::HasErrorNodes(tree, candidate)) => {
                info!(target: "llm", "LLM candidate has error nodes.\nCandidate text: {:#?}\nTree: {:#?}",
                    String::from_utf8(candidate),
                    tree.root_node().to_sexp()
                );
                None
            }
        }
    }

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
            let completion_response = self
                .complete(constructor_api::CompletionParameters {
                    messages: prompt.into(),
                    n: Some(50),
                    ..Default::default()
                })
                .await
                .into_iter()
                .inspect(|response| info!(target: "llm", "LLM response {response:#?}"));

            let completion_response_processed = completion_response
                .flat_map(|reply| reply.markdown_to_code())
                .filter_map(filter_unparsable)
                .map(|(ft, _)| ft.routine_specification())
                .collect();

            info!("completions:\t{completion_response_processed:#?}");

            Ok(completion_response_processed)
        }

        pub async fn fix_body(
            &self,
            workspace: &Workspace,
            path: &Path,
            feature_name: &FeatureName,
            error_message: String,
        ) -> Result<Option<String>> {
            let prompt = prompt::FeaturePrompt::try_new_for_feature_fixes(
                workspace,
                path,
                feature_name,
                error_message,
            )
            .await
            .with_context(|| format!("fails to make prompt to fix routine"))?
            .into();

            // Generate feature with specifications
            let completion_response = self
                .complete(constructor_api::CompletionParameters {
                    messages: prompt,
                    n: Some(5),
                    ..Default::default()
                })
                .await
                .into_iter()
                .inspect(|response| info!(target: "llm", "LLM response {response:#?}"));

            let completion_response_processed: Option<String> = completion_response
                .flat_map(|response| response.markdown_to_code())
                .filter_map(filter_unparsable)
                .filter_map(|(ft,source)| ft.body_source_unchecked(source)
                    .inspect_err(|e| info!(target: "llm", "fails to extract body of candidate feature with error: {:#?}", e))
                    .ok())
                .next();

            if completion_response_processed.is_none() {
                info!(target:"llm", "llm proposes no candidate.");
            }

            Ok(completion_response_processed)
        }

        pub async fn fixed_routine_src<'slf, 'ft: 'slf>(
            &'slf self,
            workspace: &Workspace,
            path: &Path,
            name_routine: &'ft FeatureName,
            error_message: String,
        ) -> Option<(Feature, String)> {
            let prompt = prompt::FeaturePrompt::try_new_for_feature_fixes(
                workspace,
                path,
                name_routine,
                error_message,
            )
            .await?
            .into();

            let completion_response = self
                .complete(constructor_api::CompletionParameters {
                    messages: prompt,
                    n: Some(5),
                    ..Default::default()
                })
                .await
                .into_iter()
                .inspect(|response| info!(target: "llm", "LLM response {response:#?}"));

            completion_response
                .flat_map(|response| response.markdown_to_code())
                .filter_map(filter_unparsable)
                .next()
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
            let completion_response = self
                .complete(constructor_api::CompletionParameters {
                    messages: prompt.into(),
                    n: Some(5),
                    ..Default::default()
                })
                .await
                .into_iter();

            let completion_response_processed =
                completion_response.flat_map(|reply| reply.markdown_to_code());

            Ok(todo!(
                "Process candidate code to extract class wide specifications."
            ))
        }

        /// List of LLM generated feature candidates.
        /// Each tuple in the list can be described by this pattern naming: (feature_name: String, llm_candidate_for_feature: Option<String>)
        pub async fn class_wide_fixes(
            &self,
            workspace: &Workspace,
            path: &Path,
            error_message: String,
        ) -> Vec<(FeatureName, String)> {
            let Some(class) = workspace.class(path) else {
                warn!("fails to find class at {path:#?}");
                return Vec::new();
            };

            let prompt =
                prompt::ClassPrompt::try_new_for_feature_fixes(workspace, class, error_message)
                    .await
                    .expect("fails to produce prompt for class-wide fixes.");

            let completion_response = self
                .complete(constructor_api::CompletionParameters {
                    messages: prompt.into(),
                    n: Some(5),
                    ..Default::default()
                })
                .await
                .into_iter()
                .inspect(|response| info!("LLM response: {response:#?}"));

            let maybe_code = completion_response
                .into_iter()
                .flat_map(|response| response.markdown_to_code());

            let retain_only_parsable = |candidate| {
                let mut parser = Parser::new();
                parser
                    .class_and_tree_from_source(&candidate)
                    .inspect_err(|e| {
                        warn!("fails to parse generated class:\n{candidate}\nbecause {e:#?}")
                    })
                    .ok()
                    .map(|(class, _)| (class, candidate))
            };

            fn extract_features(
                class: &Class,
                candidate_class_text: &str,
            ) -> Vec<(FeatureName, String)> {
                class
                    .features()
                    .into_iter()
                    .map(|ft| {
                        (
                            ft.name().to_owned(),
                            extract_text_within_range(candidate_class_text, ft.range())
                                .trim_end()
                                .to_string(),
                        )
                    })
                    .collect::<Vec<_>>()
            }

            maybe_code
                .filter_map(retain_only_parsable)
                .map(|(ref class, ref candidate_class_text)| {
                    extract_features(class, candidate_class_text)
                })
                .next()
                .unwrap_or_default()
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

        candidate
            .lines()
            .skip(start_row)
            .enumerate()
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
