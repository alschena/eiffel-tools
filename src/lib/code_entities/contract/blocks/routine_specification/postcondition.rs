use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;

use super::clause::Clause;
use super::*;

#[derive(Hash, Deserialize, Debug, PartialEq, Eq, Clone, JsonSchema, Default)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(
    description = "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate."
)]
pub struct Postcondition(pub Vec<Clause>);

impl Deref for Postcondition {
    type Target = Vec<Clause>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Postcondition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Contract for Postcondition {
    fn keyword() -> Keyword {
        Keyword::Ensure
    }
}

impl From<Vec<Clause>> for Postcondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}

impl Display for Postcondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.iter()
                .fold(String::from('\n'), |mut acc, elt| {
                    acc.push_str(format!("{}{}", Self::indentation_string(), elt).as_str());
                    acc
                })
                .trim_end()
        )
    }
}
