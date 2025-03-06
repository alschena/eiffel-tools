use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;

use super::clause::Clause;
use super::*;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(
    description = "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword."
)]
pub struct Precondition(Vec<Clause>);

impl Deref for Precondition {
    type Target = Vec<Clause>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Precondition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for Precondition {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl Fix for Precondition {
    fn fix_syntax(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_syntax(system_classes, current_class, current_feature));
        true
    }
    fn fix_identifiers(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| {
            clause.fix_identifiers(system_classes, current_class, current_feature)
        });
        true
    }
    fn fix_calls(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_calls(system_classes, current_class, current_feature));
        true
    }
    fn fix_repetition(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        match current_feature.preconditions() {
            Some(pr) => self.remove_redundant_clauses(pr),
            None => self.remove_self_redundant_clauses(),
        }
        true
    }
}
impl From<Vec<Clause>> for Precondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}
impl Contract for Precondition {
    fn keyword() -> Keyword {
        Keyword::Require
    }
}
impl Display for Precondition {
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
