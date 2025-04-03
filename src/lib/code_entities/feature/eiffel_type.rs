use crate::lib::code_entities::class::model::ModelExtended;
use crate::lib::code_entities::prelude::*;
use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use anyhow::anyhow;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EiffelType {
    /// The first string is the whole string.
    /// The second string is the class name.
    ClassType(String, String),
    TupleType(String),
    Anchored(String),
}
impl Display for EiffelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            EiffelType::ClassType(s, _) => s,
            EiffelType::TupleType(s) => s,
            EiffelType::Anchored(s) => s,
        };
        write!(f, "{text}")
    }
}
impl EiffelType {
    pub fn class_name(&self) -> Result<ClassName, &str> {
        match self {
            EiffelType::ClassType(_, s) => Ok(ClassName(s.to_owned())),
            EiffelType::TupleType(_) => Err("tuple type"),
            EiffelType::Anchored(_) => Err("anchored type"),
        }
    }
    pub fn class<'a, 'b: 'a>(
        &'a self,
        mut system_classes: impl Iterator<Item = &'b Class>,
    ) -> &'b Class {
        let class = system_classes
            .find(|&c| Ok(c.name()) == self.class_name().as_ref())
            .unwrap_or_else(|| {
                panic!(
                    "parameters' class name: {:?} must be in system.",
                    self.class_name()
                )
            });
        class
    }
    pub fn is_terminal_for_model(&self) -> bool {
        self.class_name()
            .is_ok_and(|class_name| class_name.is_terminal_for_model())
    }
    pub fn model_extension(&self, system_classes: &[Class]) -> ModelExtended {
        if self.is_terminal_for_model() {
            return ModelExtended::Terminal;
        }
        let Ok(class_name): Result<ClassName, _> = self.to_owned().try_into() else {
            unimplemented!("eiffel type's model extension implemented only for class types.")
        };
        class_name.model_extended(system_classes)
    }
}

impl Parse for EiffelType {
    type Error = anyhow::Error;

    fn parse_through(
        node: &Node,
        query_cursor: &mut QueryCursor,
        src: &str,
    ) -> Result<Self, Self::Error> {
        let eiffeltype = match node.kind() {
            "class_type" => {
                let query = Self::query("(class_name) @classname");
                let mut matches = query_cursor.matches(&query, *node, src.as_bytes());
                let mat = matches.next().expect("match for classname in classtype.");
                let classname_node = capture_name_to_nodes("classname", &query, mat)
                    .next()
                    .expect("capture for classname in classtype.");

                let classname = node_to_text(&classname_node, src).to_string();
                EiffelType::ClassType(node_to_text(&node, src).to_string(), classname)
            }
            "tuple_type" => EiffelType::TupleType(node_to_text(&node, src).to_string()),
            "anchored" => EiffelType::Anchored(node_to_text(&node, src).to_string()),
            _ => unreachable!(),
        };
        Ok(eiffeltype)
    }
}
impl TryFrom<EiffelType> for ClassName {
    type Error = anyhow::Error;

    fn try_from(value: EiffelType) -> Result<Self, Self::Error> {
        value.class_name().map_err(|e| anyhow!("{e}"))
    }
}

impl From<ClassName> for EiffelType {
    fn from(value: ClassName) -> Self {
        let ClassName(name) = value;
        EiffelType::ClassType(name.clone(), name)
    }
}
