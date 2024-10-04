use super::Point;
use crate::lib::tree_sitter::{self, WidthFirstTraversal};
use anyhow::{anyhow, Context};
use gemini::request::config::schema::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use serde_xml_rs::debug_expect;
use std::fmt::Display;
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
pub struct ContractClause {
    pub predicate: Predicate,
    pub tag: Tag,
}
impl ContractClause {
    fn extract_from_treesitter(node: &::tree_sitter::Node<'_>, src: &str) -> anyhow::Result<Self> {
        match node.child(0) {
            Some(tag) if tag.kind() == "tag_mark" => Ok(Self {
                predicate: Predicate {
                    predicate: src[node
                        .child(1)
                        .context("Expression follows tag")?
                        .byte_range()]
                    .to_string(),
                },
                tag: Tag {
                    tag: src[tag
                        .child(0)
                        .context("Node tag_mark must have child tag")?
                        .byte_range()]
                    .to_string(),
                },
            }),
            Some(expression) if expression.kind() == "expression" => Ok(Self {
                predicate: Predicate {
                    predicate: src[expression.byte_range()].to_string(),
                },
                tag: Tag { tag: String::new() },
            }),
            Some(_) => Err(anyhow!("Invalid child of clause")),
            None => Err(anyhow!("Empty clause")),
        }
    }
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
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PreconditionDecorated {
    precondition: Precondition,
    range: super::Range,
}
impl PreconditionDecorated {
    pub fn range(&self) -> &super::Range {
        &self.range
    }
}
impl PreconditionDecorated {
    pub(super) fn extract_from_treesitter<'a, 'b>(
        mut cursor: &mut ::tree_sitter::TreeCursor<'b>,
        src: &str,
    ) -> Result<PreconditionDecorated, anyhow::Error>
    where
        'a: 'b,
    {
        debug_assert!(cursor.node().kind() == "attribute_or_routine");
        let node = cursor.node();
        let Some(node) = node
            .children(&mut cursor)
            .find(|n| n.kind() == "precondition")
        else {
            let point = Point::from(node.range().start_point);
            return Ok(Self {
                precondition: Precondition {
                    precondition: Vec::new(),
                },
                range: super::Range {
                    start: point.clone(),
                    end: point,
                },
            });
        };
        Ok(Self {
            precondition: Precondition {
                precondition: node
                    .children(&mut cursor)
                    .map(|clause| ContractClause::extract_from_treesitter(&clause, src))
                    .collect::<anyhow::Result<Vec<ContractClause>>>()?,
            },
            range: node.range().into(),
        })
    }
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
