//! The structure of the model response candidates.
use serde::Serialize;
use std::collections::HashMap;
/// Output schema of the generated candidate text. Schemas must be a subset of the OpenAPI schema and can be objects, primitives or arrays.
/// If set, a compatible responseMimeType must also be set. Compatible MIME types: application/json: Schema for JSON response. Refer to the JSON text generation guide for more details.
#[derive(Serialize, Debug, Clone)]
pub struct ResponseSchema {
    #[serde(rename(serialize = "type"))]
    pub schema_type: SchemaType,
    /// The format of the data
    /// This is used only for primitive datatypes
    /// Supported formats: for NUMBER type: float, double for INTEGER type: int32, int64 for STRING type: enum
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// A brief description of the parameter
    /// This could contain examples of use
    /// Parameter description may be formatted as Markdown
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Indicates if the value may be null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,
    /// Possible values of the element of Type
    /// STRING with enum format
    /// For example we can define an Enum Direction as : {type:STRING, format:enum, enum:["EAST", NORTH", "SOUTH", "WEST"]}
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename(serialize = "enum"))]
    pub possibilities: Option<String>,
    /// Maximum number of the elements for Type.ARRAY.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename(serialize = "maxItems"))]
    pub max_items: Option<String>,
    ///Properties of Type.OBJECT.
    /// An object containing a list of "key": value pairs. Example: { "name": "wrench", "mass": "1.3kg", "count": "3" }.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, ResponseSchema>>,
    /// Required properties of Type.OBJECT.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    /// Schema of the elements of Type.ARRAY.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ResponseSchema>>,
}
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub enum SchemaType {
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
impl ToResponseSchema for String {
    fn to_response_schema() -> ResponseSchema {
        ResponseSchema {
            schema_type: SchemaType::String,
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
impl ToResponseSchema for f32 {
    fn to_response_schema() -> ResponseSchema {
        ResponseSchema {
            schema_type: SchemaType::Number,
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
impl ToResponseSchema for i32 {
    fn to_response_schema() -> ResponseSchema {
        ResponseSchema {
            schema_type: SchemaType::Integer,
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
impl<T> ToResponseSchema for Vec<T>
where
    T: ToResponseSchema,
{
    fn to_response_schema() -> ResponseSchema {
        ResponseSchema {
            schema_type: SchemaType::Array,
            format: None,
            description: None,
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: Some(Box::new(T::to_response_schema())),
        }
    }
}
pub trait ToResponseSchema {
    fn to_response_schema() -> ResponseSchema;
}
pub trait Described {
    fn description() -> String;
}
#[cfg(test)]
mod tests {
    use super::{SchemaType, ToResponseSchema};

    #[test]
    fn response_schema_string() {
        let schema = String::to_response_schema();
        assert_eq!(schema.schema_type, SchemaType::String)
    }
    #[test]
    fn response_schema_i32() {
        let schema = i32::to_response_schema();
        assert_eq!(schema.schema_type, SchemaType::Integer)
    }
    #[test]
    fn response_schema_f32() {
        let schema = f32::to_response_schema();
        assert_eq!(schema.schema_type, SchemaType::Number)
    }
    #[test]
    fn response_schema_vector() {
        let schema = Vec::<i32>::to_response_schema();
        assert_eq!(schema.schema_type, SchemaType::Array)
    }
}
