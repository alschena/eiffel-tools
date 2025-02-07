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

impl ModelTypes {
    pub fn new(types: Vec<EiffelType>) -> ModelTypes {
        ModelTypes(types)
    }
}

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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModelExtended<'names>(&'names Model, Vec<ModelExtended<'names>>);

impl Model {
    fn extended<'s, 'system: 's>(&'s self, system_classes: &'system [&Class]) -> ModelExtended<'s> {
        let ext: Vec<ModelExtended<'_>> = self
            .types()
            .iter()
            .flat_map(|r#type| {
                if r#type.is_terminal_for_model() {
                    Vec::new()
                } else {
                    let base_class_name = r#type.class(system_classes.iter().copied());
                    base_class_name
                        .full_model(system_classes)
                        .map(|nested_model| nested_model.extended(system_classes))
                        .collect()
                }
            })
            .collect();
        ModelExtended(self, ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_model() {
        let src_client = "
    note
        model: nested
    class NEW_CLIENT
    feature
        x: INTEGER
        nested: NEW_INTEGER
    end
    ";
        let src_supplier = "
    note
    	model: value
    class
    	NEW_INTEGER
    feature
    	value: INTEGER
    	smaller (other: NEW_INTEGER): BOOLEAN
    		do
    			Result := value < other.value
    		ensure
    			Result = (value < other.value)
    		end
    end
    ";
        let client = Class::from_source(src_client);
        let supplier = Class::from_source(src_supplier);
        let system_classes = vec![&client, &supplier];

        let ext = client.model().extended(&system_classes);

        eprintln!("{ext:?}");

        let model = ext.0;
        let nested_model = ext.1.first().expect("Nested model.").0;

        assert_eq!(model.names().len(), 1);
        assert_eq!(model.names().first().unwrap(), "nested");

        assert_eq!(model.types().len(), 1);
        assert_eq!(
            model.types().first().unwrap().class_name().unwrap(),
            "NEW_INTEGER"
        );

        assert_eq!(nested_model.names().len(), 1);
        assert_eq!(nested_model.names().first().unwrap(), "value");

        assert_eq!(nested_model.types().len(), 1);
        assert_eq!(
            nested_model.types().first().unwrap().class_name().unwrap(),
            "INTEGER"
        );
    }
}
