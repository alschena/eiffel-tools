use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::Parse;
use anyhow::anyhow;
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

    fn parse_through(
        node: &Node,
        cursor: &mut QueryCursor,
        src: &str,
    ) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "precondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, *node, src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse_through(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}
impl Parse for Block<Postcondition> {
    type Error = anyhow::Error;

    fn parse_through(
        node: &Node,
        cursor: &mut QueryCursor,
        src: &str,
    ) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "postcondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, *node, src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse_through(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}

impl Parse for RoutineSpecification {
    type Error = anyhow::Error;

    fn parse_through(
        node: &Node,
        query_cursor: &mut QueryCursor,
        src: &str,
    ) -> Result<Self, Self::Error> {
        debug_assert!(node.parent().is_none());
        let query = Self::query(
            r#"
            (feature_declaration 
            (attribute_or_routine
                (notes)? @notes
                (precondition)? @precondition
                (postcondition)? @postcondition)? @attribute_or_routine)
            "#,
        );
        query_cursor
            .matches(&query, *node, src.as_bytes())
            .next()
            .map(|query_match| {
                let mut nested_cursor = QueryCursor::new();
                let attribute_or_routine_range: Option<Range> =
                    capture_name_to_nodes("attribute_or_routine", &query, query_match)
                        .next()
                        .map(|node| node.range().into());
                let note_point_end: Option<Point> =
                    capture_name_to_nodes("notes", &query, query_match)
                        .next()
                        .map(|node| node.end_position().into());
                let precondition =
                    capture_name_to_nodes("precondition", &query, query_match)
                        .next()
                        .map_or_else(
                            || {
                                Ok(Block::new_empty(note_point_end.clone().unwrap_or_else(
                                    || {
                                        attribute_or_routine_range
                                    .as_ref()
                                    .expect("if precondition matches attribute_or_routine matches.")
                                    .end
                                    },
                                )))
                            },
                            |precondition| {
                                Block::<Precondition>::parse_through(
                                    &precondition,
                                    &mut nested_cursor,
                                    src,
                                )
                            },
                        );
                let postcondition = capture_name_to_nodes("postcondition", &query, query_match)
                    .next()
                    .map_or_else(
                        || {
                            Ok(Block::new_empty(note_point_end.unwrap_or_else(|| {
                                attribute_or_routine_range
                                    .expect(
                                        "if postcondition matches attribute_or_routine matches.",
                                    )
                                    .end
                            })))
                        },
                        |postcondition| {
                            Block::<Postcondition>::parse_through(
                                &postcondition,
                                &mut nested_cursor,
                                src,
                            )
                        },
                    );
                let precondition = precondition?.item;
                let postcondition = postcondition?.item;
                Ok(RoutineSpecification {
                    precondition,
                    postcondition,
                })
            })
            .ok_or(anyhow!(
                "fail to match routine specification with query: {query:#?}"
            ))?
    }
}

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;
    use anyhow::Result;

    #[test]
    fn parse_postcondition() -> anyhow::Result<()> {
        let src = r#"
class A feature
  x
    do
    ensure then
      True
    end
end"#;
        let class = Class::parse(src)?;
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
        Ok(())
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
