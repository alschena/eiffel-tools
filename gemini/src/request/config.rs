use serde::Serialize;
pub mod schema;
/// Configuration options for model generation and outputs.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<StopSequence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_mime_type: Option<ResponseMimeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_schema: Option<schema::ResponseSchema>,
    /// Number of generated responses to return.
    /// Currently, this value can only be set to 1.
    /// If unset, this will default to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_count: Option<i32>,
    /// The maximum number of tokens to include in a response candidate.
    /// Note: The default value varies by model, see the Model.output_token_limit attribute of the Model returned from the getModel function.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_token: Option<i32>,
    /// Controls the randomness of the output.
    /// Note: The default value varies by model, see the Model.temperature attribute of the Model returned from the getModel function.
    /// Values can range from [0.0, 2.0].
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<i32>,
    /// The maximum cumulative probability of tokens to consider when sampling.
    /// The model uses combined Top-k and Top-p (nucleus) sampling.
    /// Tokens are sorted based on their assigned probabilities so that only the most likely tokens are considered.
    /// Top-k sampling directly limits the maximum number of tokens to consider, while Nucleus sampling limits the number of tokens based on the cumulative probability.
    /// Note: The default value varies by Model and is specified by theModel.
    /// top_p attribute returned from the getModel function.
    /// An empty topK attribute indicates that the model doesn't apply top-k sampling and doesn't allow setting topK on requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<i32>,
    /// The maximum number of tokens to consider when sampling.
    /// Gemini models use Top-p (nucleus) sampling or a combination of Top-k and nucleus sampling.
    /// Top-k sampling considers the set of topK most probable tokens.
    /// Models running with nucleus sampling don't allow topK setting.
    /// Note: The default value varies by Model and is specified by theModel.
    /// top_p attribute returned from the getModel function.
    /// An empty topK attribute indicates that the model doesn't apply top-k sampling and doesn't allow setting topK on requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
}
impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            stop_sequences: None,
            response_mime_type: None,
            response_schema: None,
            candidate_count: None,
            max_output_token: None,
            temperature: None,
            top_p: None,
            top_k: None,
        }
    }
}
impl From<schema::ResponseSchema> for GenerationConfig {
    fn from(value: schema::ResponseSchema) -> Self {
        Self {
            response_schema: Some(value),
            response_mime_type: Some(ResponseMimeType::Json),
            ..Default::default()
        }
    }
}
impl GenerationConfig {
    pub fn set_stop_sequences(&mut self, stop_sequences: Option<StopSequence>) {
        self.stop_sequences = stop_sequences
    }
    pub fn set_response_mime_type(&mut self, response_mime_type: Option<ResponseMimeType>) {
        self.response_mime_type = response_mime_type
    }
    pub fn set_response_schema(&mut self, response_schema: Option<schema::ResponseSchema>) {
        self.response_schema = response_schema
    }
    pub fn set_candidate_count(&mut self, candidate_count: Option<i32>) {
        self.candidate_count = candidate_count
    }
    pub fn set_max_output_token(&mut self, max_output_token: Option<i32>) {
        self.max_output_token = max_output_token
    }
    pub fn set_temperature(&mut self, temperature: Option<i32>) {
        self.temperature = temperature
    }
    pub fn set_top_p(&mut self, top_p: Option<i32>) {
        self.top_p = top_p
    }
    pub fn set_top_k(&mut self, top_k: Option<i32>) {
        self.top_k = top_k
    }
}
impl GenerationConfig {
    pub fn stop_sequences(&self) -> &Option<StopSequence> {
        &self.stop_sequences
    }
    pub fn response_mime_type(&self) -> &Option<ResponseMimeType> {
        &self.response_mime_type
    }
    pub fn response_schema(&self) -> &Option<schema::ResponseSchema> {
        &self.response_schema
    }
    pub fn candidate_count(&self) -> &Option<i32> {
        &self.candidate_count
    }
    pub fn max_output_token(&self) -> &Option<i32> {
        &self.max_output_token
    }
    pub fn temperature(&self) -> &Option<i32> {
        &self.temperature
    }
    pub fn top_p(&self) -> &Option<i32> {
        &self.top_p
    }
    pub fn top_k(&self) -> &Option<i32> {
        &self.top_k
    }
}
/// The set of character sequences (up to 5) that will stop output generation. If specified, the API will stop at the first appearance of a stop_sequence. The stop sequence will not be included as part of the response.
#[derive(Serialize, Debug)]
pub struct StopSequence(Vec<String>);

/// MIME type of the generated candidate text.
/// Supported MIME types are: text/plain: (default) Text output.
/// application/json: JSON response in the response candidates.
/// Refer to the docs for a list of all supported text MIME types.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum ResponseMimeType {
    #[serde(rename(serialize = "application/json"))]
    Json,
    #[serde(rename(serialize = "text/plain"))]
    Text,
}

#[cfg(test)]
mod test {
    use schema::ToResponseSchema;

    use super::*;
    #[test]
    fn generation_config_from_schema() {
        let schema = String::to_response_schema();
        let generation_config = GenerationConfig::from(schema.clone());
        assert_eq!(generation_config.response_schema, Some(schema))
    }
}
