use std::cmp::{Ordering, PartialOrd};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path;
use std::path::PathBuf;
use std::str::FromStr;

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

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Class {
    name: String,
    path: Option<Location>,
    features: Vec<Box<Feature>>,
    descendants: Vec<Box<Class>>,
    ancestors: Vec<Box<Class>>,
    range: Range,
}

impl Class {
    pub(super) fn name(&self) -> &str {
        &self.name
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
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            path: None,
            features,
            descendants,
            ancestors,
            range,
        }
    }

    pub(super) fn add_feature(&mut self, feature: &Feature) {
        self.features.push(Box::new(feature.clone()))
    }

    pub(super) fn add_location(&mut self, path: &PathBuf) {
        let path = path.clone();
        self.path = Some(Location { path })
    }
}

pub struct Contract<T: ContractType>(Vec<ContractClause<T>>);
impl<T: ContractType> Deref for Contract<T> {
    type Target = Vec<ContractClause<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: ContractType> From<Vec<ContractClause<T>>> for Contract<T> {
    fn from(value: Vec<ContractClause<T>>) -> Self {
        Self(value)
    }
}
impl<T: ContractType> DerefMut for Contract<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
pub struct ContractClause<T: ContractType> {
    tag: Option<Tag>,
    predicate: Predicate,
    contract_type: PhantomData<T>,
}
impl<T: ContractType> ContractClause<T> {
    pub fn new(tag: Option<Tag>, predicate: Predicate) -> ContractClause<T> {
        let contract_type = PhantomData::<T>::default();
        ContractClause {
            tag,
            predicate,
            contract_type,
        }
    }
}
pub trait ContractType {
    fn definition() -> String;
    fn predicate_definition() -> String;
    fn tag_definition() -> String {
        "Write a valid tag clause for the Eiffel programming language.".to_string()
    }
}
pub struct Tag(String);
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag(value)
    }
}
pub struct Predicate(String);
impl From<String> for Predicate {
    fn from(value: String) -> Self {
        Predicate(value)
    }
}
impl FromStr for Predicate {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Predicate(s.to_string()))
    }
}
pub struct Precondition;
pub struct Postcondition;
impl ContractType for Precondition {
    fn definition() -> String {
        "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.".to_string()
    }
    fn predicate_definition() -> String {
        "Write a valid precondition clause for the Eiffel programming language.".to_string()
    }
}
impl ContractType for Postcondition {
    fn definition() -> String {
        "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.".to_string()
    }
    fn predicate_definition() -> String {
        "Write a valid postcondition clause for the Eiffel programming language.".to_string()
    }
}
