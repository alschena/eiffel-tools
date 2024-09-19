use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt::format;
use std::fmt::Display;
use std::future;
use std::str::FromStr;
/// Required. The content of the current conversation with the model.
/// For single-turn queries, this is a single instance.
/// For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
#[derive(Serialize, Debug)]
struct Contents {
    parts: Vec<Part>,
}
impl FromStr for Contents {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = vec![s.parse().expect("Parse string into LSP-type `Part`")];
        Ok(Self { parts })
    }
}
impl From<String> for Contents {
    fn from(value: String) -> Self {
        let parts = vec![value.into()];
        Self { parts }
    }
}
#[derive(Deserialize, Serialize, Debug)]
struct Part {
    text: String,
}
impl FromStr for Part {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Part {
            text: s.to_string(),
        })
    }
}
impl From<String> for Part {
    fn from(value: String) -> Self {
        let text = value;
        Self { text }
    }
}

/// Optional. A list of Tools the Model may use to generate the next response.
/// A Tool is a piece of code that enables the system to interact with external systems to perform an action, or set of actions, outside of knowledge and scope of the Model. Supported Tools are Function and codeExecution. Refer to the Function calling and the Code execution guides to learn more.
#[derive(Serialize, Debug)]
struct Tools;
/// Optional. Tool configuration for any Tool specified in the request. Refer to the Function calling guide for a usage example.
#[derive(Serialize, Debug)]
struct ToolConfig;
///Optional. A list of unique SafetySetting instances for blocking unsafe content.
///This will be enforced on the GenerateContentRequest.contents and GenerateContentResponse.candidates. There should not be more than one setting for each SafetyCategory type. The API will block any contents and responses that fail to meet the thresholds set by these settings. This list overrides the default settings for each SafetyCategory specified in the safetySettings. If there is no SafetySetting for a given SafetyCategory provided in the list, the API will use the default safety setting for that category. Harm categories HARM_CATEGORY_HATE_SPEECH, HARM_CATEGORY_SEXUALLY_EXPLICIT, HARM_CATEGORY_DANGEROUS_CONTENT, HARM_CATEGORY_HARASSMENT are supported. Refer to the guide for detailed information on available safety settings. Also refer to the Safety guidance to learn how to incorporate safety considerations in your AI applications.
#[derive(Serialize, Debug)]
struct SafetySetting;
/// Optional. Developer set system instruction(s). Currently, text only.
#[derive(Serialize, Debug)]
struct SystemInstruction {
    content: Content,
}
impl FromStr for SystemInstruction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let content = s.parse().expect("Parse string into LSP type `Content`");
        Ok(Self { content })
    }
}
/// Optional. Configuration options for model generation and outputs.
#[derive(Serialize, Debug)]
struct GenerationConfig {
    stop_sequences: Option<StopSequence>,
    response_mime_type: Option<ResponseMimeType>,
    response_schema: Option<ResponseSchema>,
    /// Number of generated responses to return.
    /// Currently, this value can only be set to 1. If unset, this will default to 1.
    candidate_count: Option<i32>,
    /// The maximum number of tokens to include in a response candidate.
    /// Note: The default value varies by model, see the Model.output_token_limit attribute of the Model returned from the getModel function.
    max_output_token: Option<i32>,
    /// Controls the randomness of the output.
    /// Note: The default value varies by model, see the Model.temperature attribute of the Model returned from the getModel function.
    /// Values can range from [0.0, 2.0].
    temperature: Option<i32>,
    // The maximum cumulative probability of tokens to consider when sampling.
    // The model uses combined Top-k and Top-p (nucleus) sampling.
    // Tokens are sorted based on their assigned probabilities so that only the most likely tokens are considered. Top-k sampling directly limits the maximum number of tokens to consider, while Nucleus sampling limits the number of tokens based on the cumulative probability.
    // Note: The default value varies by Model and is specified by theModel.top_p attribute returned from the getModel function. An empty topK attribute indicates that the model doesn't apply top-k sampling and doesn't allow setting topK on requests.
    top_p: Option<i32>,
    /// The maximum number of tokens to consider when sampling.
    /// Gemini models use Top-p (nucleus) sampling or a combination of Top-k and nucleus sampling. Top-k sampling considers the set of topK most probable tokens. Models running with nucleus sampling don't allow topK setting.
    /// Note: The default value varies by Model and is specified by theModel.top_p attribute returned from the getModel function. An empty topK attribute indicates that the model doesn't apply top-k sampling and doesn't allow setting topK on requests.
    top_k: Option<i32>,
}
/// Optional. The set of character sequences (up to 5) that will stop output generation. If specified, the API will stop at the first appearance of a stop_sequence. The stop sequence will not be included as part of the response.
#[derive(Serialize, Debug)]
struct StopSequence(Vec<String>);

/// Optional. MIME type of the generated candidate text. Supported MIME types are: text/plain: (default) Text output. application/json: JSON response in the response candidates. Refer to the docs for a list of all supported text MIME types.
#[derive(Serialize, Debug)]
enum ResponseMimeType {
    #[serde(rename(serialize = "application/json"))]
    Json,
    #[serde(rename(serialize = "text/plain"))]
    Text,
}

/// Optional. Output schema of the generated candidate text. Schemas must be a subset of the OpenAPI schema and can be objects, primitives or arrays.
/// If set, a compatible responseMimeType must also be set. Compatible MIME types: application/json: Schema for JSON response. Refer to the JSON text generation guide for more details.
#[derive(Serialize, Debug)]
struct ResponseSchema {
    #[serde(rename(serialize = "type"))]
    schema_type: SchemaType,
    /// The format of the data
    /// This is used only for primitive datatypes
    /// Supported formats: for NUMBER type: float, double for INTEGER type: int32, int64 for STRING type: enum
    format: Option<String>,
    /// A brief description of the parameter
    /// This could contain examples of use
    /// Parameter description may be formatted as Markdown
    description: Option<String>,
    /// Indicates if the value may be null
    nullable: Option<bool>,
    /// Possible values of the element of Type
    /// STRING with enum format
    /// For example we can define an Enum Direction as : {type:STRING, format:enum, enum:["EAST", NORTH", "SOUTH", "WEST"]}
    #[serde(rename(serialize = "enum"))]
    possibilities: Option<String>,
    /// Maximum number of the elements for Type.ARRAY.
    #[serde(rename(serialize = "maxItems"))]
    max_items: Option<String>,
    ///Properties of Type.OBJECT.
    /// An object containing a list of "key": value pairs. Example: { "name": "wrench", "mass": "1.3kg", "count": "3" }.
    properties: Option<HashMap<String, ResponseSchema>>,
    /// Optional. Required properties of Type.OBJECT.
    required: Option<Vec<String>>,
    /// Schema of the elements of Type.ARRAY.
    items: Option<Box<ResponseSchema>>,
}
#[derive(Serialize, Debug)]
enum SchemaType {
    #[serde(rename(serialize = "STRING"))]
    String,
    #[serde(rename(serialize = "NUMBER"))]
    Number,
    #[serde(rename(serialize = "INTEGER"))]
    Integer,
    #[serde(rename(serialize = "BOOLEAN"))]
    Boolean,
    #[serde(rename(serialize = "ARRAY"))]
    Array,
    #[serde(rename(serialize = "OBJECT"))]
    Object,
}
impl ResponseSchema {
    pub fn contracts() -> ResponseSchema {
        let pre = "preconditions".to_string();
        let post = "postconditions".to_string();
        let properties = Some(HashMap::from([
            (pre.clone(), ResponseSchema::preconditions()),
            (post.clone(), ResponseSchema::postconditions()),
        ]));
        let required = Some(vec![pre, post]);
        ResponseSchema {
            schema_type: SchemaType::Object,
            format: None,
            description: None,
            nullable: Some(false),
            possibilities: None,
            max_items: None,
            properties,
            required,
            items: None,
        }
    }
    fn contract() -> ResponseSchema {
        let items = Some(Box::from(ResponseSchema::contract_clause()));
        ResponseSchema {
            schema_type: SchemaType::Array,
            format: None,
            description: None,
            nullable: Some(true),
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items,
        }
    }
    fn preconditions() -> ResponseSchema {
        let mut schema = ResponseSchema::contract();
        let items = Some(Box::from(ResponseSchema::precondition_clause()));
        schema.description = todo!();
        schema.items = items;
        schema
    }
    fn postconditions() -> ResponseSchema {
        let mut schema = ResponseSchema::contract();
        schema.description = todo!();
        schema
    }
    fn contract_clause() -> ResponseSchema {
        ResponseSchema {
            schema_type: SchemaType::String,
            format: None,
            description: None,
            nullable: Some(false),
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: None,
        }
    }
    fn precondition_clause() -> ResponseSchema {
        let pre = ResponseSchema::contract_clause();
        pre.description = todo!();
        pre
    }
    fn postcondition_clause() -> ResponseSchema {
        let post = ResponseSchema::contract_clause();
        post.description = todo!();
        post
    }
}

/// Optional. The name of the content cached to use as context to serve the prediction. Format: cachedContents/{cachedContent}
#[derive(Serialize, Debug)]
struct CachedContent;
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Request {
    contents: Contents,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Tools>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<SafetySetting>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_content: Option<CachedContent>,
}
impl Request {
    async fn process(&self, config: &Config) -> Response {
        let web_client = reqwest::Client::new();
        let json_req = web_client.post(config.end_point().clone()).json(&self);
        eprintln!("{:?}", json_req);
        match json_req.send().await {
            Ok(res) => {
                let response = res
                    .json::<Response>()
                    .await
                    .expect("Decode response from Gemini");
                response
            }
            Err(_) => {
                panic!("Response of Gemini")
            }
        }
    }
    fn set_system_instruction(&mut self, system_instruction: SystemInstruction) {
        self.system_instruction = Some(system_instruction);
    }
    fn set_json_response() {}
}
impl FromStr for Request {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let contents = s.parse().expect("Parse string to LSP-type contents");
        Ok(Self {
            contents,
            tools: None,
            tool_config: None,
            safety_settings: None,
            system_instruction: None,
            generation_config: None,
            cached_content: None,
        })
    }
}
impl From<String> for Request {
    fn from(value: String) -> Self {
        let contents = value.into();
        Self {
            contents,
            tools: None,
            tool_config: None,
            safety_settings: None,
            system_instruction: None,
            generation_config: None,
            cached_content: None,
        }
    }
}
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
    parts: Vec<Part>,
    role: Option<Role>,
}
impl Default for Content {
    fn default() -> Self {
        let parts = Vec::new();
        let role = Some(Role::Model);
        Self { parts, role }
    }
}
impl FromStr for Content {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = vec![s.parse().expect("Parse string into LSP type `Part`")];
        Ok(Self {
            parts,
            ..Default::default()
        })
    }
}
#[derive(Serialize, Deserialize, Debug)]
enum Role {
    #[serde(rename(deserialize = "user"))]
    User,
    #[serde(rename(deserialize = "model"))]
    Model,
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
#[derive(Deserialize, Debug)]
struct Response {
    candidates: Candidates,
    #[serde(skip_deserializing)]
    prompt_feedback: PromptFeedback,
    #[serde(skip_deserializing)]
    usage_metadata: UsageMetadata,
}
#[derive(Debug, Clone)]
enum Mode {
    Generate,
    Stream,
}
impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Mode::Generate => write!(f, "generateContent"),
            Mode::Stream => write!(f, "streamGenerateContent"),
        }
    }
}
#[derive(Debug, Clone)]
enum Model {
    Flash,
    Pro,
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Model::Flash => write!(f, "gemini-1.5-flash"),
            Model::Pro => write!(f, "gemini-1.5-pro"),
        }
    }
}
#[derive(Debug)]
struct Config {
    token: String,
    mode: Mode,
    model: Model,
}
impl Config {
    fn new(model: Model, mode: Mode, env_var_token: String) -> Config {
        let token = env::var(env_var_token).expect("Environment variable containing gemini token.");
        Config { token, mode, model }
    }
    fn end_point(&self) -> reqwest::Url {
        let model = self.model.clone();
        let mode = self.mode.clone();
        let end_point =
            format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:{mode}?key=")
                + self.token.as_str();
        end_point
            .as_str()
            .try_into()
            .expect("Initialize config Gemini")
    }
}
impl Default for Config {
    fn default() -> Self {
        let mode = Mode::Generate;
        let model = Model::Flash;
        let env_var_token = "GOOGLE_API_KEY".to_string();
        Self::new(model, mode, env_var_token)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn serialize_request() {
        let req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        eprintln!("{:?}", serde_json::to_string(&req));
    }
    #[tokio::test]
    async fn text_response() {
        let req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        let config = Config::default();
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
    async fn json_response() {
        let req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        let config = Config::default();
        let res = req.process(&config).await;
        eprintln!("{:?}", res);
    }
}
