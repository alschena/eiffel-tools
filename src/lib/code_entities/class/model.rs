use crate::lib::code_entities::prelude::*;
use crate::lib::tree_sitter_extension::{capture_name_to_nodes, node_to_text, Parse};
use std::fmt::Display;
use std::ops::Deref;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};
// TODO accept only attributes of logical type in the model
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Model(Vec<Feature>);
impl Model {
    pub fn from_model_names(names: ModelNames, features: &Vec<Feature>) -> Model {
        Model(
            names
                .0
                .iter()
                .filter_map(|name| {
                    features
                        .iter()
                        .find(|feature| feature.name() == name)
                        .cloned()
                })
                .collect(),
        )
    }
}
impl Default for Model {
    fn default() -> Self {
        Model(Vec::new())
    }
}
impl Deref for Model {
    type Target = Vec<Feature>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Indent for Model {
    const INDENTATION_LEVEL: usize = 1;
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_text = self.0.iter().fold(String::new(), |mut acc, feature| {
            if !acc.is_empty() {
                acc.push(',');
                acc.push(' ');
            }
            acc.push_str(format!("{feature}").as_str());
            acc
        });
        write!(f, "{display_text}")
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModelNames(Vec<String>);
impl Parse for ModelNames {
    type Error = anyhow::Error;

    fn parse(node: &Node, query_cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        let name_query = Self::query(
            r#"(class_declaration
            (notes (note_entry
                (tag) @tag
                value: (_) @id
                ("," value: (_) @id)*))
            (#eq? @tag "model"))"#,
        );

        let mut matches = query_cursor.matches(&name_query, *node, src.as_bytes());

        let mut names: Vec<String> = Vec::new();
        while let Some(mat) = matches.next() {
            capture_name_to_nodes("id", &name_query, mat)
                .for_each(|node| names.push(node_to_text(&node, src).to_string()));
        }

        Ok(ModelNames(names))
    }
}
