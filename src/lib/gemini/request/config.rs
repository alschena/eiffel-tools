use serde::Serialize;
use std::collections::HashMap;
/// The content of the current conversation with the model.
/// For single-turn queries, this is a single instance.
/// For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
const DESCRIPTION_PRECONDITION: &str = "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.";

const DESCRIPTION_POSTCONDITION: &str = "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.
        ";
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
    pub fn format(&self) -> &Option<String> {
        &self.format
    }
    pub fn description(&self) -> &Option<String> {
        &self.description
    }
    pub fn nullable(&self) -> &Option<bool> {
        &self.nullable
    }
    pub fn possibilities(&self) -> &Option<String> {
        &self.possibilities
    }
    pub fn max_items(&self) -> &Option<String> {
        &self.max_items
    }
    pub fn properties(&self) -> &Option<HashMap<String, ResponseSchema>> {
        &self.properties
    }
    pub fn required(&self) -> &Option<Vec<String>> {
        &self.required
    }
    pub fn items(&self) -> &Option<Box<ResponseSchema>> {
        &self.items
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
        schema.description = Some(DESCRIPTION_PRECONDITION.to_string());
        schema.items = items;
        schema
    }
    fn postconditions() -> ResponseSchema {
        let mut schema = ResponseSchema::contract();
        schema.description = Some(DESCRIPTION_POSTCONDITION.to_string());
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
