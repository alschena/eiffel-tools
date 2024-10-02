use gemini::request::config::schema::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;
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
