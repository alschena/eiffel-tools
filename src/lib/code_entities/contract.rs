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
#[derive(Debug, PartialEq, Eq, Clone)]
/// Wraps an optional contract clause adding whereabouts informations.
/// If the `item` is None, the range start and end coincide where the contract clause would be added.
pub struct ContractBlock<T> {
    pub item: Option<T>,
    pub range: Range,
    pub keyword: ContractKeyword,
}
impl<T: Indent> Indent for ContractBlock<T> {
    const INDENTATION_LEVEL: u32 = T::INDENTATION_LEVEL - 1;
}
impl<T: Display + Indent> Display for ContractBlock<T> {
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
pub enum ContractKeyword {
    Require,
    RequireThen,
    Ensure,
    EnsureElse,
    Invariant,
}
impl Display for ContractKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self {
            ContractKeyword::Require => "require",
            ContractKeyword::RequireThen => "require then",
            ContractKeyword::Ensure => "ensure",
            ContractKeyword::EnsureElse => "ensure else",
            ContractKeyword::Invariant => "invariant",
        };
        write!(f, "{}", content)
    }
}
impl<T> ContractBlock<T> {
    pub fn item(&self) -> &Option<T> {
        &self.item
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct ContractClause {
    pub predicate: Predicate,
    pub tag: Tag,
}
impl Parse for ContractClause {
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
impl Display for ContractClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}\n", self.tag, self.predicate)
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
impl Indent for Precondition {
    const INDENTATION_LEVEL: u32 = 3;
}
impl Parse for ContractBlock<Precondition> {
    type Error = anyhow::Error;
    fn parse(
        attribute_or_routine: &Node,
        src: &str,
    ) -> Result<ContractBlock<Precondition>, anyhow::Error> {
        debug_assert!(attribute_or_routine.kind() == "attribute_or_routine");

        let mut binding = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = Query::new(lang, "(precondition) @x").unwrap();
        let mut precondition_captures =
            binding.captures(&query, attribute_or_routine.clone(), src.as_bytes());
        let precondition_cap = precondition_captures.next();
        let node = match precondition_cap {
            Some(x) => x.0.captures[0].node,
            None => {
                let point = &Point::from(attribute_or_routine.range().start_point);

                return Ok(Self {
                    item: None,
                    range: Range {
                        start: point.clone(),
                        end: point.clone(),
                    },
                    keyword: ContractKeyword::Require,
                });
            }
        };

        let query = Query::new(lang, "(assertion_clause (expression)) @x").unwrap();
        let mut assertion_clause_matches =
            binding.matches(&query, attribute_or_routine.clone(), src.as_bytes());

        let mut precondition: Vec<ContractClause> = Vec::new();
        while let Some(mat) = assertion_clause_matches.next() {
            for cap in mat.captures {
                precondition.push(ContractClause::parse(&cap.node, src)?)
            }
        }

        Ok(Self {
            item: Some(Precondition { precondition }),
            range: node.range().into(),
            keyword: ContractKeyword::Require,
        })
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct Postcondition {
    pub postcondition: Vec<ContractClause>,
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
    use ContractClause;

    #[test]
    fn extract_contract_clause() {
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
        let clause = ContractClause::parse(&node, &src).expect("Parse feature");
        assert_eq!(clause.tag, Tag::from(String::new()));
        assert_eq!(clause.predicate, Predicate::new("True".to_string()));
    }
    #[test]
    fn extract_precondition() {
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

        let precondition = <ContractBlock<Precondition>>::parse(&node, &src)
            .expect("fails to parse precondition.");
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
}
