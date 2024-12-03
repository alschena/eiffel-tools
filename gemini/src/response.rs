use crate::ToResponseSchema;
use std::ops::{Deref, DerefMut};
use tracing::warn;

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
    pub fn candidates(&self) -> &Candidates {
        &self.candidates
    }
    pub fn parsable_content<'a>(&'a self) -> impl Iterator<Item = &str> + 'a {
        self.candidates.parsable_content()
    }
    pub fn parsed<'a, 'de, T: ToResponseSchema + Deserialize<'de>>(
        &'a self,
    ) -> impl Iterator<Item = T> + 'a
    where
        'a: 'de,
    {
        self.candidates.parse()
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
pub struct Candidates(Vec<Candidate>);
impl Candidates {
    pub fn parsable_content<'a>(&'a self) -> impl Iterator<Item = &str> + 'a {
        self.iter()
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
    pub fn parse<'a, 'de, T: ToResponseSchema + Deserialize<'de>>(
        &'a self,
    ) -> impl Iterator<Item = T> + 'a
    where
        'a: 'de,
    {
        self.parsable_content()
            .filter_map(|item| match serde_json::from_str::<T>(item) {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!("fails to parse {item:?} with error: {e:?}");
                    None
                }
            })
    }
}
impl Deref for Candidates {
    type Target = Vec<Candidate>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Candidates {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
#[derive(Deserialize, Debug, Clone)]
pub struct Candidate {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
#[derive(Deserialize, Debug, Clone)]
enum FinishReason {}
/// List of ratings for the safety of a response candidate. There is at most one rating per category.
#[derive(Deserialize, Debug, Default, Clone)]
struct SafetyRatings {}
/// Output only. Citation information for model-generated candidate. This field may be populated with recitation information for any text included in the content. These are passages that are "recited" from copyrighted material in the foundational LLM's training data.
#[derive(Deserialize, Debug, Default, Clone)]
struct CitationMetadata {}
/// Output only. Token count for this candidate.
#[derive(Deserialize, Debug, Default, Clone)]
struct TokenCount {}
/// Output only. Attribution information for sources that contributed to a grounded answer. This field is populated for GenerateAnswer calls.
#[derive(Deserialize, Debug, Default, Clone)]
struct GroundingAttributions {}
/// Output only. Index of the candidate in the list of response candidates.
#[derive(Deserialize, Debug, Default, Clone)]
struct Index {}

#[cfg(test)]
mod test {
    use super::super::model;
    use super::super::request::{self, config};
    use super::*;
    use anyhow::Result;
    use std::str::FromStr;
}
