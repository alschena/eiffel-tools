use super::class::Class;
use super::shared::*;
use gemini::lib::request::config::schema::{ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Feature {
    name: String,
    visibility: FeatureVisibility,
    range: Range,
}
impl Feature {
    pub fn from_name_and_range(name: String, range: Range) -> Feature {
        let visibility = FeatureVisibility::Private;
        Feature {
            name,
            visibility,
            range,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
}
