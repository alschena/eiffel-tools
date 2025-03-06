use crate::lib::tree_sitter_extension::Parse;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
use streaming_iterator::StreamingIterator;
use tree_sitter::Node;
use tree_sitter::QueryCursor;

use super::clause::Clause;
use super::*;

mod routine_specification;

pub use routine_specification::Postcondition;
pub use routine_specification::Precondition;
pub use routine_specification::RoutineSpecification;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
/// Wraps an optional contract clause adding whereabouts informations.
/// If the `item` is None, the range start and end coincide where the contract clause would be added.
pub struct Block<T> {
    pub item: T,
    pub range: Range,
}
impl<T: Contract> Block<T> {
    pub fn item(&self) -> &T {
        &self.item
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn new(item: T, range: Range) -> Self {
        Self { item, range }
    }
}
impl<T: Contract + Default> Block<T> {
    pub fn new_empty(point: Point) -> Self {
        Self {
            item: T::default(),
            range: Range::new_collapsed(point),
        }
    }
}
impl<T: Indent> Indent for Block<T> {
    const INDENTATION_LEVEL: usize = T::INDENTATION_LEVEL - 1;
}
impl<T: Display + Indent + Contract + Deref<Target = Vec<Clause>>> Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.item().is_empty() {
            write!(f, "")
        } else {
            write!(
                f,
                "{}{}\n{}",
                T::keyword(),
                &self.item,
                Self::indentation_string(),
            )
        }
    }
}
impl Parse for Block<Precondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "precondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, node.clone(), src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}
impl Parse for Block<Postcondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "postcondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, node.clone(), src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;
    use anyhow::Result;

    #[test]
    fn fix_repetition_in_preconditions() {
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
        let sc = vec![Class::from_source(src)];
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
            .is_some_and(|p| p.predicate == Predicate::new("f = r")))
    }
    #[test]
    fn parse_precondition() {
        let src = r#"
class A feature
  x
    require
      True
    do
    end
end"#;
        let class = Class::from_source(src);
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
    }
    #[test]
    fn parse_postcondition() {
        let src = r#"
class A feature
  x
    do
    ensure then
      True
    end
end"#;
        let class = Class::from_source(src);
        let feature = class.features().first().expect("first feature is `x`.");

        let postcondition = feature
            .postconditions()
            .expect("postcondition block with trivial postcondition.");

        let clause = postcondition
            .first()
            .expect("trivial postcondition clause.");
        assert_eq!(postcondition.len(), 1);

        assert_eq!(&clause.predicate, &Predicate::new("True".to_string()));
        assert_eq!(&clause.tag, &Tag::new(""));
    }
    #[test]
    fn display_precondition_block() {
        let empty_block: Block<Precondition> = Block::new_empty(Point { row: 0, column: 0 });
        let simple_block: Block<Precondition> = Block::new(
            vec![Clause {
                tag: Tag::default(),
                predicate: Predicate::default(),
            }]
            .into(),
            Range::new(Point { row: 0, column: 0 }, Point { row: 0, column: 4 }),
        );
        assert_eq!(format!("{empty_block}"), "");
        assert_eq!(
            format!("{simple_block}"),
            "require\n\t\t\tdefault: True\n\t\t"
        );
    }
}
