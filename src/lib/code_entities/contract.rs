use super::prelude::*;
use crate::lib::tree_sitter_extension::Parse;
use ::tree_sitter::{Node, Query, QueryCursor, QueryMatch};
use anyhow::{anyhow, Context};
use gemini::request::config::schema::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use serde_xml_rs::debug_expect;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
trait Type {
    const TREE_NODE_KIND: &str;
    const DEFAULT_KEYWORD: Keyword;
    const POSITIONED: Positioned;
}
#[derive(Debug, PartialEq, Eq, Clone)]
/// Wraps an optional contract clause adding whereabouts informations.
/// If the `item` is None, the range start and end coincide where the contract clause would be added.
pub struct Block<T> {
    pub item: Option<T>,
    pub range: Range,
    pub keyword: Keyword,
}
impl<T: Indent> Indent for Block<T> {
    const INDENTATION_LEVEL: u32 = T::INDENTATION_LEVEL - 1;
}
impl<T: Display + Indent> Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}\n{}",
            &self.keyword,
            match &self.item {
                Some(c) => format!("{}", c),
                None => "True".to_owned(),
            },
            Self::indentation_string(),
        )
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Keyword {
    Require,
    RequireThen,
    Ensure,
    EnsureElse,
    Invariant,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Positioned {
    Prefix,
    Postfix,
}
impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self {
            Keyword::Require => "require",
            Keyword::RequireThen => "require then",
            Keyword::Ensure => "ensure",
            Keyword::EnsureElse => "ensure else",
            Keyword::Invariant => "invariant",
        };
        write!(f, "{}", content)
    }
}
impl<T> Block<T> {
    pub fn item(&self) -> &Option<T> {
        &self.item
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Clause {
    pub predicate: Predicate,
    pub tag: Tag,
}
impl Parse for Clause {
    type Error = anyhow::Error;
    fn parse(assertion_clause: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert_eq!(assertion_clause.kind(), "assertion_clause");
        debug_assert!(assertion_clause.child_count() > 0);

        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let clause_query =
            ::tree_sitter::Query::new(lang, "((tag_mark (tag) @tag)? (expression) @expr)").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures =
            binding.captures(&clause_query, assertion_clause.clone(), src.as_bytes());

        match captures.next() {
            Some(&(ref m, _)) => {
                let tag_node = m.nodes_for_capture_index(0).next();
                let expression_node = m.nodes_for_capture_index(1).next().unwrap();

                let tag: Tag = match tag_node {
                    Some(n) => src[n.byte_range()].to_string().into(),
                    None => String::new().into(),
                };

                let predicate = Predicate::new(src[expression_node.byte_range()].to_string());

                Ok(Self { predicate, tag })
            }
            None => Err(anyhow!("Wrong arguments, should match")),
        }
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}\n", self.tag, self.predicate)
    }
}
impl Clause {
    pub fn new(tag: Tag, predicate: Predicate) -> Clause {
        Clause { tag, predicate }
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
    pub precondition: Vec<Clause>,
}
impl From<Vec<Clause>> for Precondition {
    fn from(value: Vec<Clause>) -> Self {
        Self {
            precondition: value,
        }
    }
}
impl Indent for Precondition {
    const INDENTATION_LEVEL: u32 = 3;
}
impl Type for Precondition {
    const TREE_NODE_KIND: &str = "precondition";
    const DEFAULT_KEYWORD: Keyword = Keyword::Require;
    const POSITIONED: Positioned = Positioned::Prefix;
}
impl<T: Type + From<Vec<Clause>>> Parse for Block<T> {
    type Error = anyhow::Error;
    fn parse(attribute_or_routine: &Node, src: &str) -> Result<Block<T>, anyhow::Error> {
        debug_assert!(attribute_or_routine.kind() == "attribute_or_routine");

        let mut binding = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = Query::new(lang, format!("({}) @x", T::TREE_NODE_KIND).as_str()).unwrap();
        let mut precondition_captures =
            binding.captures(&query, attribute_or_routine.clone(), src.as_bytes());
        let precondition_cap = precondition_captures.next();
        let node = match precondition_cap {
            Some(x) => x.0.captures[0].node,
            None => {
                let point = match T::POSITIONED {
                    Positioned::Prefix => &Point::from(attribute_or_routine.range().start_point),
                    Positioned::Postfix => &Point::from(attribute_or_routine.range().end_point),
                };
                return Ok(Self {
                    item: None,
                    range: Range {
                        start: point.clone(),
                        end: point.clone(),
                    },
                    keyword: T::DEFAULT_KEYWORD,
                });
            }
        };

        let query = Query::new(lang, "(assertion_clause (expression)) @x").unwrap();
        let mut assertion_clause_matches =
            binding.matches(&query, attribute_or_routine.clone(), src.as_bytes());

        let mut clauses: Vec<Clause> = Vec::new();
        while let Some(mat) = assertion_clause_matches.next() {
            for cap in mat.captures {
                clauses.push(Clause::parse(&cap.node, src)?)
            }
        }

        Ok(Self {
            item: Some(clauses.into()),
            range: node.range().into(),
            keyword: T::DEFAULT_KEYWORD,
        })
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Postcondition {
    pub postcondition: Vec<Clause>,
}
impl From<Vec<Clause>> for Postcondition {
    fn from(value: Vec<Clause>) -> Self {
        Self {
            postcondition: value,
        }
    }
}
impl Type for Postcondition {
    const TREE_NODE_KIND: &str = "postcondition";
    const DEFAULT_KEYWORD: Keyword = Keyword::Ensure;
    const POSITIONED: Positioned = Positioned::Postfix;
}
impl Indent for Postcondition {
    const INDENTATION_LEVEL: u32 = 3;
}
impl Display for Precondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.precondition
                .iter()
                .fold(String::from('\n'), |acc, elt| {
                    format!("{acc}{}{elt}", Self::indentation_string())
                        .trim_end()
                        .to_owned()
                })
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
#[cfg(test)]
mod tests {
    use super::*;
    use Clause;

    #[test]
    fn parse_clause() {
        let src = r#"
class A feature
  x
    require
      True
    do
    end

  y
    require else
    do
    end
end"#;
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();
        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(assertion_clause) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;
        let clause = Clause::parse(&node, &src).expect("Parse feature");
        assert_eq!(clause.tag, Tag::from(String::new()));
        assert_eq!(clause.predicate, Predicate::new("True".to_string()));
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
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(attribute_or_routine) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;

        let precondition =
            <Block<Precondition>>::parse(&node, &src).expect("fails to parse precondition.");
        let predicate = Predicate::new("True".to_string());
        let tag = Tag { tag: String::new() };
        let clause = precondition
            .item
            .clone()
            .expect("fails to find non-empty precondition")
            .precondition
            .pop()
            .expect("Parse clause");
        assert_eq!(clause.predicate, predicate);
        assert_eq!(clause.tag, tag);
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
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(attribute_or_routine) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;

        let postcondition =
            <Block<Postcondition>>::parse(&node, &src).expect("fails to parse postcondition.");
        let predicate = Predicate::new("True".to_string());
        let tag = Tag { tag: String::new() };
        let clause = postcondition
            .item
            .clone()
            .expect("fails to find non-empty postcondition")
            .postcondition
            .pop()
            .expect("Parse clause");
        assert_eq!(clause.predicate, predicate);
        assert_eq!(clause.tag, tag);
    }
}
