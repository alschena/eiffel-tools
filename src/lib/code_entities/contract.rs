use gemini::request::config::schema::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::fmt::Display;
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct ContractClause {
    pub predicate: Predicate,
    pub tag: Tag,
}
impl Display for ContractClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.tag, self.predicate)
    }
}
impl ContractClause {
    pub fn new(tag: Tag, predicate: Predicate) -> ContractClause {
        ContractClause { tag, predicate }
    }
}
#[derive(Deserialize, Clone, ToResponseSchema, Debug, PartialEq, Eq)]
pub struct Tag {
    pub tag: String,
}
impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag)
    }
}
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag { tag: value }
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Predicate {
    pub predicate: String,
}
impl Display for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.predicate)
    }
}
impl Predicate {
    fn new(s: String) -> Predicate {
        Predicate { predicate: s }
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Precondition {
    pub precondition: Vec<ContractClause>,
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Postcondition {
    pub postcondition: Vec<ContractClause>,
}
impl Display for Precondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.precondition
                .iter()
                .fold(String::new(), |acc, elt| { format!("{acc}\n{elt}") })
        )
    }
}
impl Display for Postcondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.postcondition
                .iter()
                .fold(String::new(), |acc, elt| { format!("{acc}\n{elt}") })
        )
    }
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
