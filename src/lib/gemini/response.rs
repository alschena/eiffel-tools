use super::model_config;
use super::request;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Response {
    candidates: Candidates,
    #[serde(skip_deserializing)]
    prompt_feedback: PromptFeedback,
    #[serde(skip_deserializing)]
    usage_metadata: UsageMetadata,
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
    role: Option<model_config::Role>,
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
    use super::super::model_config;
    use super::super::request;
    use super::*;

    #[tokio::test]
    async fn text_response() {
        let req = request::Request::from(
            "Write a story about turles from the prospective of a frog.".to_string(),
        );
        let config = model_config::Config::default();
        let web_client = reqwest::Client::new();
        let json_req = web_client.post(config.end_point().clone()).json(&req);
        eprintln!("{:?}", json_req);
        let res = match json_req.send().await {
            Ok(res) => res.text().await.expect("Decode response from Gemini"),
            Err(_) => {
                panic!("Response of Gemini")
            }
        };
        eprintln!("{:?}", res);
    }
    #[tokio::test]
    async fn simple_json_response() {
        let req = request::Request::from(
            "Write a story about turtles from the prospective of a frog.".to_string(),
        );
        let config = model_config::Config::default();
        let res = req.process(&config).await;
    }
    #[tokio::test]
    async fn format_contract_response_as_json() {
        let mut req = request::Request::from("Write preconditions and postconditions for the routine `minimum (x: INTEGER, y: INTEGER): INTEGER`.".to_string());
        let mut generation_config = request::GenerationConfig::default();
        generation_config.set_response_mime_type(Some(request::ResponseMimeType::Json));
        // assert!(generation_config.response_mime_type == Some(request::ResponseMimeType::Json));
        generation_config.set_response_schema(Some(request::ResponseSchema::contracts()));
        req.set_generation_config(Some(generation_config));
        eprintln!("{:?}", req);
        let server_config = model_config::Config::default();
        let res = req.process(&server_config).await;
        eprintln!("{:?}", res);
        assert!(false)
    }
}
