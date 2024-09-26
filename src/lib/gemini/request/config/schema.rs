use async_lsp::Result;
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
const DESCRIPTION_PRECONDITION_CLAUSE: &str =
    "Write a valid precondition clause for the Eiffel programming language.";
const DESCRIPTION_POSTCONDITION_CLAUSE: &str =
    "Write a valid postcondition clause for the Eiffel programming language.";
const DESCRIPTION_TAG_CLAUSE: &str =
    "Write a valid tag clause for the Eiffel programming language.";
/// Output schema of the generated candidate text. Schemas must be a subset of the OpenAPI schema and can be objects, primitives or arrays.
/// If set, a compatible responseMimeType must also be set. Compatible MIME types: application/json: Schema for JSON response. Refer to the JSON text generation guide for more details.
#[derive(Serialize, Debug, Clone)]
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
#[derive(Serialize, Debug, Clone)]
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
impl TryFrom<&serde_json::Value> for SchemaType {
    type Error = &'static str;
    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        match value {
            serde_json::Value::Null => Err("Null is an invalid example of Schematype"),
            serde_json::Value::Bool(_) => Ok(SchemaType::Boolean),
            serde_json::Value::Number(_) => Ok(SchemaType::Boolean),
            serde_json::Value::String(_) => Ok(SchemaType::String),
            serde_json::Value::Array(_) => Ok(SchemaType::Array),
            serde_json::Value::Object(_) => Ok(SchemaType::Object),
        }
    }
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
// TODO add test
impl TryFrom<&serde_json::Value> for ResponseSchema {
    type Error = &'static str;

    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        let schema_type: SchemaType = value
            .try_into()
            .expect("Null is an invalid example of ResponseSchema");
        let format: Option<String> = None;
        let description: Option<String> = None;
        let nullable: Option<bool> = None;
        let possibilities: Option<String> = None;
        let max_items: Option<String> = None;
        let mut properties: Option<HashMap<String, ResponseSchema>> = None;
        let mut required: Option<Vec<String>> = None;
        let mut items: Option<Box<ResponseSchema>> = None;
        match value {
            serde_json::Value::Array(entries) => {
                if entries.is_empty() {
                    return Err("Empty array is an invalid example of ResponseSchema");
                } else {
                    items = Some(Box::new(
                        entries
                            .first()
                            .expect("The vector must be non-empty")
                            .try_into()
                            .expect("Valid items of array type"),
                    ))
                }
            }
            serde_json::Value::Object(attributes) => {
                let mut p = HashMap::new();
                let mut r = Vec::new();
                attributes.iter().for_each(|(k, v)| {
                    p.insert(k.clone(), v.try_into().expect("In the range of the HashMap a value which is convertible to a ResponseSchema"));
                    r.push(k.clone())
                });
                properties = Some(p);
                required = Some(r);
            }
            _ => {}
        }
        Ok(ResponseSchema {
            schema_type,
            format,
            description,
            nullable,
            possibilities,
            max_items,
            properties,
            required,
            items,
        })
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
// TODO refactor this implementation using traits.
// TODO get the schema from a serde::Value.
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
    fn contract(clause_schema: ResponseSchema) -> ResponseSchema {
        let items = Some(Box::from(clause_schema));
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
        let pre_schema = ResponseSchema::tagged_precondition_clause();
        let mut schema = ResponseSchema::contract(pre_schema.clone());
        let items = Some(Box::from(pre_schema));
        schema.description = Some(DESCRIPTION_PRECONDITION.to_string());
        schema.items = items;
        schema
    }
    fn postconditions() -> ResponseSchema {
        let post_schema = ResponseSchema::tagged_postcondition_clause();
        let mut schema = ResponseSchema::contract(post_schema.clone());
        let items = Some(Box::from(post_schema));
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
        pre.description = Some(DESCRIPTION_PRECONDITION_CLAUSE.to_string());
        pre
    }
    fn postcondition_clause() -> ResponseSchema {
        let mut post = ResponseSchema::contract_clause();
        post.description = Some(DESCRIPTION_POSTCONDITION_CLAUSE.to_string());
        post
    }
    fn tag() -> ResponseSchema {
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
    fn tagged_precondition_clause() -> ResponseSchema {
        let pre = "preconditions".to_string();
        let tag = "tag".to_string();
        let properties = Some(HashMap::from([
            (pre.clone(), ResponseSchema::precondition_clause()),
            (tag.clone(), ResponseSchema::tag()),
        ]));
        let required = Some(vec![tag, pre]);
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

    fn tagged_postcondition_clause() -> ResponseSchema {
        let post = "postconditions".to_string();
        let tag = "tag".to_string();
        let properties = Some(HashMap::from([
            (post.clone(), ResponseSchema::postcondition_clause()),
            (tag.clone(), ResponseSchema::tag()),
        ]));
        let required = Some(vec![tag, post]);
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
}
