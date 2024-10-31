use super::model;
use super::request;
use serde::{Deserialize, Serialize};
use std::iter::Iterator;

#[derive(Deserialize, Debug)]
pub struct Response {
    candidates: Candidates,
    #[serde(skip_deserializing)]
    prompt_feedback: PromptFeedback,
    #[serde(skip_deserializing)]
    usage_metadata: UsageMetadata,
}
impl Response {
    pub fn parsable_content<'a>(&'a self) -> impl Iterator<Item = &str> + 'a {
        self.candidates
            .0
            .iter()
            .filter_map(|c| {
                let content = &c.content;
                match content.role {
                    Some(model::Role::User) => None,
                    Some(model::Role::Model) => Some(content.text_parts()),
                    None => None,
                }
            })
            .flatten()
    }
}
/// Returns the prompt's feedback related to the content filters.
#[derive(Deserialize, Debug, Default)]
struct PromptFeedback {
    block_reason: Option<BlockReason>,
    safety_ratings: SafetyRatings,
}
#[derive(Deserialize, Debug)]
enum BlockReason {}
/// Output only. Metadata on the generation requests' token usage.
#[derive(Deserialize, Debug, Default)]
struct UsageMetadata;
/// Candidate responses from the model.
#[derive(Deserialize, Debug)]
struct Candidates(Vec<Candidate>);
#[derive(Deserialize, Debug)]
struct Candidate {
    content: Content,
    #[serde(skip)]
    finish_reason: Option<FinishReason>,
    #[serde(skip)]
    safety_ratings: SafetyRatings,
    #[serde(skip)]
    citation_metadata: CitationMetadata,
    #[serde(skip)]
    token_count: TokenCount,
    #[serde(skip)]
    grounding_attributions: GroundingAttributions,
    #[serde(skip)]
    index: Index,
}

#[derive(Serialize, Deserialize, Debug)]
struct Content {
    parts: Vec<request::Part>,
    role: Option<model::Role>,
}
impl Content {
    pub fn text_parts<'a>(&'a self) -> impl Iterator<Item = &str> + 'a {
        self.parts.iter().map(|p| p.text())
    }
}
/// Optional. Output only. The reason why the model stopped generating tokens. If empty, the model has not stopped generating tokens.
#[derive(Deserialize, Debug)]
enum FinishReason {}
/// List of ratings for the safety of a response candidate. There is at most one rating per category.
#[derive(Deserialize, Debug, Default)]
struct SafetyRatings {}
/// Output only. Citation information for model-generated candidate. This field may be populated with recitation information for any text included in the content. These are passages that are "recited" from copyrighted material in the foundational LLM's training data.
#[derive(Deserialize, Debug, Default)]
struct CitationMetadata {}
/// Output only. Token count for this candidate.
#[derive(Deserialize, Debug, Default)]
struct TokenCount {}
/// Output only. Attribution information for sources that contributed to a grounded answer. This field is populated for GenerateAnswer calls.
#[derive(Deserialize, Debug, Default)]
struct GroundingAttributions {}
/// Output only. Index of the candidate in the list of response candidates.
#[derive(Deserialize, Debug, Default)]
struct Index {}

#[cfg(test)]
mod test {
    use super::super::model;
    use super::super::request::{self, config};
    use super::*;
    use anyhow::Result;
    use std::str::FromStr;
}
