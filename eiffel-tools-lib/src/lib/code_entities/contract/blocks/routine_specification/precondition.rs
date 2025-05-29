use crate::lib::eiffel_source::Indent;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;

use super::clause::Clause;
use super::*;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash, JsonSchema, Default)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(
    description = "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword."
)]
pub struct Precondition(pub Vec<Clause>);

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

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;
    use crate::lib::parser::Parser;
    use anyhow::Result;

    fn class(source: &str) -> Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(source)
    }

    #[test]
    fn parse_precondition() -> anyhow::Result<()> {
        let src = r#"
class A feature
  x
    require
      True
    do
    end
end"#;
        let class = class(src)?;
        let feature = class
            .features()
            .first()
            .expect("class `A` has feature `x`.");
        eprintln!("{feature:?}");

        assert!(feature.supports_precondition_block());
        assert!(feature.supports_postcondition_block());

        let precondition = feature
            .preconditions()
            .expect("feature has precondition block.");

        let clause = precondition
            .first()
            .expect("precondition block has trivial assertion clause.");

        let predicate = &clause.predicate;
        let tag = &clause.tag;

        assert_eq!(*predicate, Predicate::new("True".to_string()));
        assert_eq!(*tag, Tag::new(""));
        Ok(())
    }
}
