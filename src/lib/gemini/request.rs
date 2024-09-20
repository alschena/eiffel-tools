use super::model_config;
use super::response;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
/// The content of the current conversation with the model.
/// For single-turn queries, this is a single instance.
/// For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
#[derive(Serialize, Debug)]
pub struct Contents {
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
/// For single-turn queries, it is the user's latest written request.
/// For multi-turn queries like chat, each part collects a snapshot of the conversation history or the latest request.
#[derive(Deserialize, Serialize, Debug)]
pub(super) struct Part {
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

/// A list of Tools the Model may use to generate the next response.
/// A Tool is a piece of code that enables the system to interact with external systems to perform an action, or set of actions, outside of knowledge and scope of the Model.
/// Supported Tools are Function and codeExecution.
/// Refer to the Function calling and the Code execution guides to learn more.
#[derive(Serialize, Debug)]
pub struct Tools;
/// Tool configuration for any Tool specified in the request.
/// Refer to the Function calling guide for a usage example.
#[derive(Serialize, Debug)]
pub struct ToolConfig;
/// A list of unique SafetySetting instances for blocking unsafe content.
/// This will be enforced on the GenerateContentRequest.
/// contents and GenerateContentResponse.
/// candidates.
/// There should not be more than one setting for each SafetyCategory type.
/// The API will block any contents and responses that fail to meet the thresholds set by these settings.
/// This list overrides the default settings for each SafetyCategory specified in the safetySettings.
/// If there is no SafetySetting for a given SafetyCategory provided in the list, the API will use the default safety setting for that category.
/// Harm categories HARM_CATEGORY_HATE_SPEECH, HARM_CATEGORY_SEXUALLY_EXPLICIT, HARM_CATEGORY_DANGEROUS_CONTENT, HARM_CATEGORY_HARASSMENT are supported.
/// Refer to the guide for detailed information on available safety settings.
/// Also refer to the Safety guidance to learn how to incorporate safety considerations in your AI applications.
#[derive(Serialize, Debug)]
pub struct SafetySetting;
/// Developer set system instruction(s).
/// Currently, text only.
#[derive(Serialize, Debug)]
pub struct SystemInstruction {
    content: Content,
}
impl FromStr for SystemInstruction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let content = s.parse().expect("Parse string into LSP type `Content`");
        Ok(Self { content })
    }
}
/// Configuration options for model generation and outputs.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<StopSequence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_mime_type: Option<ResponseMimeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_schema: Option<ResponseSchema>,
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
impl GenerationConfig {
    pub fn set_stop_sequences(&mut self, stop_sequences: Option<StopSequence>) {
        self.stop_sequences = stop_sequences
    }
    pub fn set_response_mime_type(&mut self, response_mime_type: Option<ResponseMimeType>) {
        self.response_mime_type = response_mime_type
    }
    pub fn set_response_schema(&mut self, response_schema: Option<ResponseSchema>) {
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
    pub fn response_schema(&self) -> &Option<ResponseSchema> {
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

/// Output schema of the generated candidate text. Schemas must be a subset of the OpenAPI schema and can be objects, primitives or arrays.
/// If set, a compatible responseMimeType must also be set. Compatible MIME types: application/json: Schema for JSON response. Refer to the JSON text generation guide for more details.
#[derive(Serialize, Debug)]
pub struct ResponseSchema {
    #[serde(rename(serialize = "type"))]
    schema_type: SchemaType,
    /// The format of the data
    /// This is used only for primitive datatypes
    /// Supported formats: for NUMBER type: float, double for INTEGER type: int32, int64 for STRING type: enum
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    /// A brief description of the parameter
    /// This could contain examples of use
    /// Parameter description may be formatted as Markdown
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Indicates if the value may be null
    #[serde(skip_serializing_if = "Option::is_none")]
    nullable: Option<bool>,
    /// Possible values of the element of Type
    /// STRING with enum format
    /// For example we can define an Enum Direction as : {type:STRING, format:enum, enum:["EAST", NORTH", "SOUTH", "WEST"]}
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename(serialize = "enum"))]
    possibilities: Option<String>,
    /// Maximum number of the elements for Type.ARRAY.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename(serialize = "maxItems"))]
    max_items: Option<String>,
    ///Properties of Type.OBJECT.
    /// An object containing a list of "key": value pairs. Example: { "name": "wrench", "mass": "1.3kg", "count": "3" }.
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<HashMap<String, ResponseSchema>>,
    /// Required properties of Type.OBJECT.
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<Vec<String>>,
    /// Schema of the elements of Type.ARRAY.
    #[serde(skip_serializing_if = "Option::is_none")]
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
impl From<SchemaType> for ResponseSchema {
    fn from(value: SchemaType) -> Self {
        ResponseSchema {
            schema_type: value,
            format: None,
            description: None,
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: None,
        }
    }
}
impl ResponseSchema {
    pub fn set_format(&mut self, format: Option<String>) {
        self.format = format
    }
    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description
    }
    pub fn set_nullable(&mut self, nullable: Option<bool>) {
        self.nullable = nullable
    }
    pub fn set_possibilities(&mut self, possibilities: Option<String>) {
        self.possibilities = possibilities
    }
    pub fn set_max_items(&mut self, max_items: Option<String>) {
        self.max_items = max_items
    }
    pub fn set_properties(&mut self, properties: Option<HashMap<String, ResponseSchema>>) {
        self.properties = properties
    }
    pub fn set_required(&mut self, required: Option<Vec<String>>) {
        self.required = required
    }
    pub fn set_items(&mut self, items: Option<Box<ResponseSchema>>) {
        self.items = items
    }
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
        schema.description = Some("Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword. ".to_string());
        schema.items = items;
        schema
    }
    fn postconditions() -> ResponseSchema {
        let mut schema = ResponseSchema::contract();
        schema.description = Some("Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.
        ".to_string());
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
        let mut pre = ResponseSchema::contract_clause();
        pre.description = Some(
            "Write a valid precondition clause for the Eiffel programming language.".to_string(),
        );
        pre
    }
    fn postcondition_clause() -> ResponseSchema {
        let mut post = ResponseSchema::contract_clause();
        post.description = Some(
            "Write a valid precondition clause for the Eiffel programming language.".to_string(),
        );
        post
    }
}

/// The name of the content cached to use as context to serve the prediction. Format: cachedContents/{cachedContent}
#[derive(Serialize, Debug)]
pub struct CachedContent;
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Request {
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
    pub fn set_contents(&mut self, contents: Contents) {
        self.contents = contents;
    }
    pub fn set_tools(&mut self, tools: Option<Tools>) {
        self.tools = tools;
    }
    pub fn set_tool_config(&mut self, tool_config: Option<ToolConfig>) {
        self.tool_config = tool_config;
    }
    pub fn set_safety_settings(&mut self, safety_settings: Option<Vec<SafetySetting>>) {
        self.safety_settings = safety_settings;
    }
    pub fn set_system_instruction(&mut self, system_instruction: Option<SystemInstruction>) {
        self.system_instruction = system_instruction;
    }
    pub fn set_generation_config(&mut self, generation_config: Option<GenerationConfig>) {
        self.generation_config = generation_config;
    }
    pub fn set_cached_content(&mut self, cached_content: Option<CachedContent>) {
        self.cached_content = cached_content;
    }
}
impl Request {
    pub async fn process(&self, config: &model_config::Config) -> response::Response {
        let web_client = reqwest::Client::new();
        let json_req = web_client.post(config.end_point().clone()).json(&self);
        eprintln!("{:?}", json_req);
        match json_req.send().await {
            Ok(res) => {
                let response = res
                    .json::<response::Response>()
                    .await
                    .expect("Decode response from Gemini");
                response
            }
            Err(_) => {
                panic!("Response of Gemini")
            }
        }
    }
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

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn serialize_simple_request() {
        let req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        eprintln!("{:?}", serde_json::to_string(&req));
    }
    #[test]
    fn serialize_request_json_config() {
        let mut req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        let mut generation_config = GenerationConfig::default();
        generation_config.set_response_mime_type(Some(ResponseMimeType::Json));
        assert!(generation_config.response_mime_type == Some(ResponseMimeType::Json));
        generation_config.set_response_schema(Some(ResponseSchema::contracts()));
        req.set_generation_config(Some(generation_config));
        eprintln!("{:?}", serde_json::to_string(&req));
    }
}
