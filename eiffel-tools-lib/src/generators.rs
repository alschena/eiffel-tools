use crate::code_entities::prelude::*;
use crate::parser::Parser;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use contract::RoutineSpecification;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;
use tracing::warn;

mod prompt;

mod constructor_api;
mod backend;
mod openrouter;

pub use backend::LlmBackend;
pub use prompt::FixPromptParts;

/// One LLM choice tried during a fix attempt, with full API metadata.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    /// Raw text returned by the LLM for this choice.
    pub content: String,
    /// Finish reason reported by the API (e.g. "stop", "length").
    pub finish_reason: Option<String>,
    /// Whether this suggestion was accepted (parsed as valid Eiffel and applied).
    pub accepted: bool,
    /// Why this suggestion was rejected; present only when `accepted` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    // --- API response metadata (same for every choice in the same call) ---
    pub api_response_id: String,
    pub model: String,
    pub created_at: i64,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    /// Any extra usage/provider fields from the API payload (pricing, routing, etc.).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Result of one LLM fix call, always populated regardless of outcome.
#[derive(Debug)]
pub struct LlmFixResult {
    /// Parsed feature + its full source text when the LLM produced valid Eiffel.
    pub success: Option<(Feature, String)>,
    /// The prompt that was sent; `None` only when prompt construction failed.
    pub prompt: Option<String>,
    /// Every LLM choice tried, in order, with outcome and full API metadata.
    pub suggestions: Vec<Suggestion>,
    /// Top-level error when `success` is `None` (API failure, prompt build failure, etc.).
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct Generators {
    llms: Vec<Arc<dyn LlmBackend>>,
    model: String,
    pub rate_limited: Arc<AtomicBool>,
}

impl Default for Generators {
    fn default() -> Self {
        Self {
            llms: Vec::new(),
            model: "openai/gpt-oss-120b:free".to_string(), // free model on OpenRouter
            rate_limited: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Generators {
    pub async fn add_constructor(&mut self) {
        let Ok(llm) = constructor_api::Llm::try_new().await else {
            warn!("fail to create LLM via constructor API");
            return;
        };
        self.llms.push(Arc::new(llm));
    }

    pub fn add_openrouter(&mut self) {
        let Ok(llm) = openrouter::Llm::try_new() else {
            warn!("fail to create LLM via OpenRouter: OPENROUTER_TOKEN not set?");
            return;
        };
        self.llms.push(Arc::new(llm));
    }

    pub fn with_model(model: String) -> Self {
        Self {
            llms: Vec::new(),
            model,
            rate_limited: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a Generators instance with a model specified by name string.
    pub fn with_model_name(model_name: &str) -> Self {
        Self::with_model(model_name.to_string())
    }

    /// Get the model name as a string.
    pub fn model_name(&self) -> &str {
        &self.model
    }

    fn default_completion_parameters(&self) -> constructor_api::CompletionParameters {
        constructor_api::CompletionParameters {
            model: self.model.clone(),
            ..Default::default()
        }
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

        let rate_limited = self.rate_limited.clone();
        completion_response.into_iter().filter_map(move |rs| {
            match rs {
                Ok(response) => Some(response),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("429") || msg.contains("Rate limit") || msg.contains("401") || msg.contains("403") {
                        warn!(target:"llm", "LLM rate limit / auth error — flagging for graceful exit: {msg}");
                        rate_limited.store(true, Ordering::SeqCst);
                        return None;
                    }
                    warn!(target:"llm", "An LLM request has returned the error: {e:#?}");
                    None
                }
            }
        })
    }
}

mod feature_focused {
    use super::*;
    use crate::parser::Parsed;

    /// Try to parse a candidate string as an Eiffel feature.
    /// Returns `Ok((feature, source))` on success, `Err(reason)` on rejection.
    fn try_parse_suggestion(candidate: String) -> Result<(Feature, String), String> {
        match Parser::default().to_feature(&candidate) {
            Err(e) => {
                let reason = format!("parse error: {e:#?}");
                warn!(target: "llm", "Rejected suggestion — {reason}");
                Err(reason)
            }
            Ok(Parsed::Correct(val)) => {
                info!(target: "llm", "Accepted suggestion (parsable Eiffel)");
                Ok((val, candidate))
            }
            Ok(Parsed::HasErrorNodes(tree, _)) => {
                let reason = format!("tree-sitter error nodes: {}", tree.root_node().to_sexp());
                warn!(target: "llm", "Rejected suggestion — {reason}");
                Err(reason)
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
            let mut params = self.default_completion_parameters();
            params.messages = prompt.into();
            params.n = Some(50);
            let completion_response = self
                .complete(params)
                .await
                .into_iter()
                .inspect(|response| info!(target: "llm", "LLM response {response:#?}"));

            let completion_response_processed = completion_response
                .flat_map(|reply| reply.markdown_to_code())
                .filter_map(|c| try_parse_suggestion(c).ok())
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
                prompt::FixPromptParts::default(),
            )
            .await
            .with_context(|| "fails to make prompt to fix routine".to_string())?
            .into();

            // Generate feature with specifications
            let mut params = self.default_completion_parameters();
            params.messages = prompt;
            params.n = Some(5);
            let completion_response = self
                .complete(params)
                .await
                .into_iter()
                .inspect(|response| info!(target: "llm", "LLM response {response:#?}"));

            let completion_response_processed: Option<String> = completion_response
                .flat_map(|response| response.markdown_to_code())
                .filter_map(|c| try_parse_suggestion(c).ok())
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
            parts: FixPromptParts,
        ) -> LlmFixResult {
            let feature_prompt = match prompt::FeaturePrompt::try_new_for_feature_fixes(
                workspace,
                path,
                name_routine,
                error_message,
                parts,
            )
            .await
            {
                Some(p) => p,
                None => return LlmFixResult {
                    success: None,
                    prompt: None,
                    suggestions: Vec::new(),
                    error: Some("Failed to construct prompt — feature not found in workspace".into()),
                },
            };

            let prompt_string = feature_prompt.to_string();
            let prompt_messages: Vec<constructor_api::MessageOut> = feature_prompt.into();

            let mut params = self.default_completion_parameters();
            params.messages = prompt_messages;
            params.n = Some(5);

            let responses: Vec<_> = self
                .complete(params)
                .await
                .into_iter()
                .collect();

            if responses.is_empty() {
                warn!(target: "llm", "No LLM responses received — all API requests failed");
                return LlmFixResult {
                    success: None,
                    prompt: Some(prompt_string),
                    suggestions: Vec::new(),
                    error: Some("No LLM responses — all API requests failed (see llm.log)".into()),
                };
            }

            let mut suggestions: Vec<Suggestion> = Vec::new();

            for response in responses.iter() {
                let meta_id    = response.id.clone();
                let meta_model = response.model.clone();
                let meta_ts    = response.created;
                let meta_pt    = response.usage.prompt_tokens;
                let meta_ct    = response.usage.completion_tokens;
                let meta_tt    = response.usage.total_tokens;
                let meta_extra = {
                    let mut m = response.usage.extra.clone();
                    m.extend(response.extra.clone());
                    m
                };

                for choice in response.choices.iter() {
                    let finish_reason = choice.finish_reason.clone();
                    // Try candidates in priority order: fenced block first, raw code second.
                    let candidates = constructor_api::CompletionResponse::code_candidates(
                        &choice.message.content,
                    );
                    let parse_result = candidates
                        .into_iter()
                        .find_map(|code| try_parse_suggestion(code).ok());

                    match parse_result {
                        Some((feature, full_source)) => {
                            suggestions.push(Suggestion {
                                content: choice.message.content.clone(),
                                finish_reason,
                                accepted: true,
                                rejection_reason: None,
                                api_response_id: meta_id.clone(),
                                model: meta_model.clone(),
                                created_at: meta_ts,
                                prompt_tokens: meta_pt,
                                completion_tokens: meta_ct,
                                total_tokens: meta_tt,
                                extra: meta_extra.clone(),
                            });
                            return LlmFixResult {
                                success: Some((feature, full_source)),
                                prompt: Some(prompt_string),
                                suggestions,
                                error: None,
                            };
                        }
                        None => {
                            // Record the rejection reason from the first (highest-priority) candidate.
                            let first_reason = constructor_api::CompletionResponse::code_candidates(
                                &choice.message.content,
                            )
                            .into_iter()
                            .find_map(|code| try_parse_suggestion(code).err())
                            .unwrap_or_else(|| "no code candidates extracted".to_string());
                            suggestions.push(Suggestion {
                                content: choice.message.content.clone(),
                                finish_reason,
                                accepted: false,
                                rejection_reason: Some(first_reason),
                                api_response_id: meta_id.clone(),
                                model: meta_model.clone(),
                                created_at: meta_ts,
                                prompt_tokens: meta_pt,
                                completion_tokens: meta_ct,
                                total_tokens: meta_tt,
                                extra: meta_extra.clone(),
                            });
                        }
                    }
                }
            }

            let n = suggestions.len();
            warn!(target: "llm", "All {n} suggestion(s) rejected — no parsable Eiffel code produced");
            LlmFixResult {
                success: None,
                prompt: Some(prompt_string),
                suggestions,
                error: Some(format!("All {n} suggestion(s) rejected — no parsable Eiffel code")),
            }
        }
    }
}

mod class_wide {
    use super::*;

    impl Generators {
        #[allow(unused_variables)]
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
            let mut params = self.default_completion_parameters();
            params.messages = prompt.into();
            params.n = Some(5);
            let completion_response = self
                .complete(params)
                .await
                .into_iter();

            let completion_response_processed =
                completion_response.flat_map(|reply| reply.markdown_to_code());

            todo!("Process candidate code to extract class wide specifications.")
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

            let mut params = self.default_completion_parameters();
            params.messages = prompt.into();
            params.n = Some(5);
            let completion_response = self
                .complete(params)
                .await
                .into_iter()
                .inspect(|response| info!("LLM response: {response:#?}"));

            let maybe_code = completion_response
                .into_iter()
                .flat_map(|response| response.markdown_to_code());

            let retain_only_parsable = |candidate| {
                let mut parser = Parser::default();
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
                    .iter()
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
        Generators {
            llms: Vec::new(),
            model: "claude-sonnet-4-0".to_string(),
            rate_limited: Arc::new(AtomicBool::new(false)),
        }
    }
}
