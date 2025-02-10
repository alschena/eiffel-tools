use crate::lib::code_entities::prelude::*;
use crate::lib::tree_sitter_extension::{capture_name_to_nodes, node_to_text, Parse};
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
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

impl DerefMut for ModelNames {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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

impl DerefMut for ModelTypes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
    pub fn append(&mut self, other: &mut Model) {
        self.0.append(&mut other.0);
        self.1.append(&mut other.1);
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

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ModelExtended(Model, Vec<Option<ModelExtended>>);

impl ModelExtended {
    fn append<'model>(&'model mut self, other: &'model mut ModelExtended) {
        self.0.append(&mut other.0);
        self.1.append(&mut other.1);
    }
}

impl Model {
    pub fn extended<'s, 'system: 's>(self, system_classes: &'system [&Class]) -> ModelExtended {
        let ext: Vec<_> = self
            .types()
            .iter()
            .map(|r#type| {
                if r#type.is_terminal_for_model() {
                    None
                } else {
                    let base_class_name = r#type.class(system_classes.iter().copied());
                    base_class_name
                        .full_model(system_classes)
                        .cloned()
                        .map(|nested_model| nested_model.extended(system_classes))
                        .reduce(|mut acc, ref mut ext| {
                            acc.append(ext);
                            acc
                        })
                }
            })
            .collect();
        ModelExtended(self, ext)
    }
}

impl ModelExtended {
    fn model(&self) -> &Model {
        &self.0
    }
    fn fmt_helper(&self, indent: usize) -> String {
        self.0
            .names()
            .iter()
            .zip(self.0.types().iter())
            .zip(self.1.iter())
            .fold(String::new(), |mut acc, ((name, r#type), ext)| {
                if !acc.is_empty() {
                    acc.push(';');
                    acc.push('\n');
                }

                (0..indent).for_each(|_| acc.push('\t'));

                acc.push_str(format!("{name}: {type}").as_str());

                if let Some(ext) = ext {
                    acc.push('\n');
                    acc.push_str(ext.fmt_helper(indent + 1).as_str());
                }
                acc
            })
    }
}

impl Display for ModelExtended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.fmt_helper(0);
        write!(f, "{text}")
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

        let ext = client.model().clone().extended(&system_classes);

        eprintln!("{ext:?}");

        let model = ext.0;

        let nested_model = ext.1.first().expect("Nested model.").as_ref().unwrap();
        let nested_model = nested_model.model();

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

    #[test]
    fn extended_model2() {
        let src_client = "
    note
        model: x
    class NEW_CLIENT
    feature
        x: INTEGER
    end
    ";
        let src_client2 = "
    note
        model: nested
    class NEW_CLIENT_2
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
        let client2 = Class::from_source(src_client2);
        let supplier = Class::from_source(src_supplier);
        let system_classes = vec![&client, &client2, &supplier];

        let ext = client.model().clone().extended(&system_classes);
        let ext2 = client2.model().clone().extended(&system_classes);

        eprintln!("f: {ext:?}");
        eprintln!("s: {ext2:?}");

        let model = ext.0;

        let mut ext_models = ext.1.iter();
        let nested = ext_models.next();

        assert!(nested.unwrap().is_none());

        assert_eq!(model.names().len(), 1);
        assert_eq!(model.names().first().unwrap(), "x");

        assert_eq!(model.types().len(), 1);
        assert_eq!(
            model.types().first().unwrap().class_name().unwrap(),
            "INTEGER"
        );

        let model = ext2.0;
        let nested = ext2.1.iter().next().unwrap().as_ref().unwrap().model();

        assert_eq!(model.names().len(), 1);
        assert_eq!(model.names().first().unwrap(), "nested");

        assert_eq!(model.types().len(), 1);
        assert_eq!(
            model.types().first().unwrap().class_name().unwrap(),
            "NEW_INTEGER"
        );

        assert_eq!(nested.names().len(), 1);
        assert_eq!(nested.names().first().unwrap(), "value");

        assert_eq!(nested.types().len(), 1);
        assert_eq!(
            nested.types().first().unwrap().class_name().unwrap(),
            "INTEGER"
        );
    }

    #[test]
    fn display_extended_model() {
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

        let mut ext_model = client.full_extended_model(&system_classes);
        let first = ext_model.next().unwrap();
        assert!(ext_model.next().is_none());
        eprintln!("{first}");
        assert_eq!(format!("{first}"), "nested: NEW_INTEGER\n\tvalue: INTEGER");
    }
}
