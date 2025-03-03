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
            .map(|name| {
                let f = features.into_iter().find(|feature| feature.name() == name);
                if f.is_none() {
                    warn!("Model feature not found {name:?}")
                }
                f
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

#[derive(Debug, PartialEq, Eq, Hash, Default)]
pub enum ModelExtended {
    Terminal,
    Recursive,
    #[default]
    IsEmpty,
    Model {
        names: ModelNames,
        types: ModelTypes,
        extension: Vec<ModelExtended>,
    },
}

impl Model {
    pub fn extended<'s, 'system: 's>(self, system_classes: &'system [Class]) -> ModelExtended {
        self.extended_helper(&mut ModelTypes::new(Vec::new()), system_classes)
    }
    fn extended_helper(self, visited: &mut ModelTypes, system_classes: &[Class]) -> ModelExtended {
        let Model(names, types) = self;
        if names.is_empty() {
            return ModelExtended::IsEmpty;
        }
        let extension: Vec<_> = types
            .iter()
            .map(|t| {
                if t.is_terminal_for_model() {
                    return ModelExtended::Terminal;
                }
                if visited.iter().find(|&visited| t == visited).is_some() {
                    return ModelExtended::Recursive;
                }

                visited.extend(types.iter().cloned());

                let base_class_name = t.class(system_classes.iter());
                base_class_name
                    .model_with_inheritance(system_classes)
                    .extended_helper(visited, system_classes)
            })
            .collect();

        ModelExtended::Model {
            names,
            types,
            extension,
        }
    }
}

impl ModelExtended {
    pub fn fmt_indented(&self, indent: usize) -> String {
        let mut text = String::new();
        (0..indent).for_each(|_| text.push('\t'));

        match self {
            ModelExtended::Terminal => {
                text.push_str("the model is implemented in Boogie.\n");
            }
            ModelExtended::Recursive => text.push_str("the model is recursive.\n"),
            ModelExtended::IsEmpty => text.push_str("the model is empty.\n"),
            ModelExtended::Model {
                names,
                types,
                extension,
            } => {
                text.push_str("the model is: ");
                for ((name, ty), ext) in names.iter().zip(types.iter()).zip(extension) {
                    text.push_str(format!("{name}: {ty}").as_str());
                    text.push('\n');
                    text.push_str(ext.fmt_indented(indent + 1).as_str());
                }
            }
        };
        text
    }
}

impl Indent for ModelExtended {
    const INDENTATION_LEVEL: usize = Model::INDENTATION_LEVEL;
}

impl Display for ModelExtended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.fmt_indented(0);
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
        let system_classes = vec![
            Class::from_source(src_client),
            Class::from_source(src_supplier),
        ];
        let client = &system_classes[0];

        let top_model = client.model().clone().extended(&system_classes);

        eprintln!("{top_model:?}");

        let ModelExtended::Model {
            names,
            types,
            extension,
        } = top_model
        else {
            panic!("top model must be populated.")
        };

        assert_eq!(names.len(), 1);
        assert_eq!(names.first().unwrap(), "nested");
        assert_eq!(types.len(), 1);
        assert_eq!(types.first().unwrap().class_name().unwrap(), "NEW_INTEGER");

        let Some(ModelExtended::Model {
            names,
            types,
            extension,
        }) = extension.first()
        else {
            panic!("nested model must be populated.")
        };

        assert_eq!(names.len(), 1);
        assert_eq!(names.first().unwrap(), "value");

        assert_eq!(types.len(), 1);
        assert_eq!(types.first().unwrap().class_name().unwrap(), "INTEGER");
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
        let system_classes = vec![
            Class::from_source(src_client),
            Class::from_source(src_client2),
            Class::from_source(src_supplier),
        ];

        let client = &system_classes[0];
        let client2 = &system_classes[1];

        let ModelExtended::Model {
            names,
            types,
            extension,
        } = client.model().clone().extended(&system_classes)
        else {
            panic!("client must have a populated model.")
        };

        assert_eq!(extension.len(), 1);
        assert_eq!(extension.first().unwrap(), &ModelExtended::Terminal);
        assert_eq!(names.len(), 1);
        assert_eq!(names.first().unwrap(), "x");

        assert_eq!(types.len(), 1);
        assert_eq!(types.first().unwrap().class_name().unwrap(), "INTEGER");

        let ModelExtended::Model {
            names,
            types,
            extension,
        } = client2.model().clone().extended(&system_classes)
        else {
            panic!("client must have a populated model.")
        };

        assert_eq!(names.len(), 1);
        assert_eq!(names.first().unwrap(), "nested");
        assert_eq!(types.len(), 1);
        assert_eq!(types.first().unwrap().class_name().unwrap(), "NEW_INTEGER");
        assert_eq!(extension.len(), 1);

        let Some(ModelExtended::Model {
            names,
            types,
            extension: _,
        }) = extension.first()
        else {
            panic!("client must have a populated model.")
        };
        assert_eq!((names).len(), 1);
        assert_eq!((names).first().unwrap(), "value");
        assert_eq!((types).len(), 1);
        assert_eq!((types).first().unwrap().class_name().unwrap(), "INTEGER");
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
        let system_classes = vec![
            Class::from_source(src_client),
            Class::from_source(src_supplier),
        ];

        let client = &system_classes[0];

        let model = client.model_extended(&system_classes);
        assert_eq!(
            format!("{model}"),
            "the model is: nested: NEW_INTEGER\n\tthe model is: value: INTEGER\n\t\tthe model is implemented in Boogie.\n"
        );
    }
}
