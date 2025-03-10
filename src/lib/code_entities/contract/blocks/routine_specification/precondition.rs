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

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;

    #[test]
    fn fix_repetition_in_preconditions() -> anyhow::Result<()> {
        let src = "
            class
                A
            feature
                x (f: BOOLEAN, r: BOOLEAN): BOOLEAN
                    require
                        t: f = True
                    do
                        Result := f
                    ensure
                        res: Result = True
                    end
            end
        ";
        let sc = vec![Class::parse(src)?];
        let c = &sc[0];
        let f = c.features().first().unwrap();

        let mut fp: Precondition = vec![
            Clause::new(Tag::new("s"), Predicate::new("f = r")),
            Clause::new(Tag::new("ss"), Predicate::new("f = r")),
        ]
        .into();

        assert!(fp.fix(&sc, &c, f));
        assert!(fp
            .first()
            .is_some_and(|p| p.predicate == Predicate::new("f = r")));
        Ok(())
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
        let class = Class::parse(src)?;
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
