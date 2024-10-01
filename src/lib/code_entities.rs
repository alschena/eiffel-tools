use gemini::lib::request::config::schema::{ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Point {
    pub(super) row: usize,
    pub(super) column: usize,
}
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.row < other.row {
            Some(Ordering::Less)
        } else if other.row < self.row {
            Some(Ordering::Greater)
        } else {
            if self.column < other.column {
                Some(Ordering::Less)
            } else if other.column < self.column {
                Some(Ordering::Greater)
            } else {
                Some(Ordering::Equal)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Range {
    pub(super) start: Point,
    pub(super) end: Point,
}
impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (s, o) if s.start <= o.start && s.end >= o.end => Some(Ordering::Greater),
            (s, o) if s.start > o.start && s.end < o.end => Some(Ordering::Less),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Location {
    pub(super) path: path::PathBuf,
}

impl From<&str> for Location {
    fn from(value: &str) -> Self {
        let path = value.into();
        Self { path }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Feature {
    name: String,
    visibility: FeatureVisibility,
    range: Range,
}
impl Feature {
    pub(super) fn from_name_and_range(name: String, range: Range) -> Feature {
        let visibility = FeatureVisibility::Private;
        Feature {
            name,
            visibility,
            range,
        }
    }
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn range(&self) -> &Range {
        &self.range
    }
}

// TODO accept only attributes of logical type in the model
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Model(pub Vec<Feature>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Class {
    name: String,
    path: Option<Location>,
    model: Model,
    features: Vec<Box<Feature>>,
    descendants: Vec<Box<Class>>,
    ancestors: Vec<Box<Class>>,
    range: Range,
}

impl Class {
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn model(&self) -> &Model {
        &self.model
    }
    pub(super) fn features(&self) -> &Vec<Box<Feature>> {
        &self.features
    }
    pub(super) fn into_features(self) -> Vec<Box<Feature>> {
        self.features
    }
    pub(super) fn range(&self) -> &Range {
        &self.range
    }
    pub(super) fn location(&self) -> Option<&Location> {
        match &self.path {
            None => None,
            Some(file) => Some(&file),
        }
    }
    pub(super) fn from_name_range(name: String, range: Range) -> Class {
        let model = Model(Vec::new());
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            path: None,
            model,
            features,
            descendants,
            ancestors,
            range,
        }
    }

    pub(super) fn add_feature(&mut self, feature: &Feature) {
        self.features.push(Box::new(feature.clone()))
    }

    pub(super) fn add_model(&mut self, model: &Model) {
        self.model = model.clone()
    }

    pub(super) fn add_location(&mut self, path: &PathBuf) {
        let path = path.clone();
        self.path = Some(Location { path })
    }
}
#[derive(Deserialize, ToResponseSchema)]
pub struct ContractClause {
    pub tag: Tag,
    pub predicate: Predicate,
}
impl ContractClause {
    pub fn new(tag: Tag, predicate: Predicate) -> ContractClause {
        ContractClause { tag, predicate }
    }
}
#[derive(Deserialize, ToResponseSchema)]
pub struct Tag {
    pub tag: String,
}
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag { tag: value }
    }
}
#[derive(Deserialize, ToResponseSchema)]
pub struct Predicate {
    pub predicate: String,
}
impl Predicate {
    fn new(s: String) -> Predicate {
        Predicate { predicate: s }
    }
}
#[derive(Deserialize, ToResponseSchema)]
pub struct Precondition {
    pub precondition: Vec<ContractClause>,
}
#[derive(Deserialize, ToResponseSchema)]
pub struct Postcondition {
    pub postcondition: Vec<ContractClause>,
}
pub trait Described {
    fn description() -> String;
}
impl Described for Tag {
    fn description() -> String {
        "A valid tag clause for the Eiffel programming language.".to_string()
    }
}
impl Described for Predicate {
    fn description() -> String {
        "A valid boolean expression for the Eiffel programming language.".to_string()
    }
}
impl Described for Precondition {
    fn description() -> String {
        "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.".to_string()
    }
}
impl Described for Postcondition {
    fn description() -> String {
        "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.".to_string()
    }
}
#[cfg(test)]
mod tests {}
