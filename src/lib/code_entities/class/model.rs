use crate::lib::code_entities::prelude::*;
use crate::lib::tree_sitter_extension::{capture_name_to_nodes, node_to_text, Parse};
use std::fmt::Display;
use std::ops::Deref;
use streaming_iterator::StreamingIterator;
use tracing::warn;
use tree_sitter::{Node, QueryCursor};
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ModelNames(Vec<String>);

impl Extend<String> for ModelNames {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        iter.into_iter().for_each(|s| self.0.push(s))
    }
}

impl Deref for ModelNames {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ModelTypes(Vec<EiffelType>);

impl Extend<EiffelType> for ModelTypes {
    fn extend<T: IntoIterator<Item = EiffelType>>(&mut self, iter: T) {
        iter.into_iter().for_each(|t| self.0.push(t));
    }
}

impl Deref for ModelTypes {
    type Target = Vec<EiffelType>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Model(ModelNames, ModelTypes);

impl Model {
    pub fn names(&self) -> &ModelNames {
        &self.0
    }
    pub fn types(&self) -> &ModelTypes {
        &self.1
    }
    pub fn from_model_names<'feature>(
        names: ModelNames,
        features: impl IntoIterator<Item = &'feature Feature> + Copy,
    ) -> Model {
        let (names, types) = names
            .iter()
            .map(|name| features.into_iter().find(|feature| feature.name() == name))
            .inspect(|feature| {
                if feature.is_none() {
                    warn!("Model feature not found {feature:?}")
                }
            })
            .zip(names.iter())
            .filter_map(|(feature, name)| feature.map(|feature| (feature, name)))
            .map(|(feature, name)| (feature.return_type(), name))
            .inspect(|(feature, _)| {
                if feature.is_none() {
                    warn!("Model feature {feature:?} cannot be a procedure")
                }
            })
            .filter_map(|(feature, name)| feature.map(|f| (name.clone(), f.clone())))
            .collect::<(ModelNames, ModelTypes)>();
        Model(names, types)
    }
}
impl Indent for Model {
    const INDENTATION_LEVEL: usize = 1;
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_text = self.names().iter().zip(self.types().iter()).fold(
            String::new(),
            |mut acc, (name, r#type)| {
                if !acc.is_empty() {
                    acc.push(',');
                    acc.push(' ');
                }
                acc.push_str(format!("{name}: {type}").as_str());
                acc
            },
        );
        write!(f, "{display_text}")
    }
}
