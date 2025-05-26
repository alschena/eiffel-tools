use crate::lib::code_entities::prelude::*;
use anyhow::ensure;
use anyhow::Result;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ModelNames(Vec<String>);

impl<T: ToString> From<Vec<T>> for ModelNames {
    fn from(value: Vec<T>) -> Self {
        let content = value.iter().map(|name| name.to_string()).collect();
        Self(content)
    }
}

impl ModelNames {
    pub fn new(names: Vec<String>) -> Self {
        Self(names)
    }
}

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

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ModelTypes(Vec<EiffelType>);

impl ModelTypes {
    pub fn new(types: Vec<EiffelType>) -> ModelTypes {
        ModelTypes(types)
    }
}

impl FromIterator<EiffelType> for ModelTypes {
    fn from_iter<T: IntoIterator<Item = EiffelType>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
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
    pub fn is_empty(&self) -> bool {
        assert_eq!(self.0.len(), self.1.len());
        self.0.is_empty()
    }
    pub fn names(&self) -> &ModelNames {
        &self.0
    }
    pub fn types(&self) -> &ModelTypes {
        &self.1
    }
    pub fn append(&mut self, other: &mut Model) {
        self.0.append(&mut other.0);
        self.1.append(&mut other.1);
    }
    pub fn try_from_names_and_features<'ft>(
        names: ModelNames,
        features: impl IntoIterator<Item = &'ft Feature>,
    ) -> Result<Self> {
        let types: ModelTypes = features
            .into_iter()
            .filter(|ft| {
                names
                    .iter()
                    .find(|&model_name| model_name == ft.name())
                    .is_some()
            })
            .filter_map(|ft| ft.return_type())
            .cloned()
            .collect();

        ensure!(names.len() == types.len(),"fails to find a type for each model feature.\n\tnames received: {names:#?}\n\ttypes found: {types:#?}");

        Ok(Model(names, types))
    }
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_text = self.names().iter().zip(self.types().iter()).fold(
            String::new(),
            |mut acc, (name, ty)| {
                acc.push_str(format!("{name}: {ty}").as_str());
                acc.push('\n');
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
    pub fn fmt_verbose_indented(&self, indent: usize) -> String {
        let mut text = String::new();
        (0..indent).for_each(|_| text.push('\t'));

        match self {
            ModelExtended::Terminal => {
                text.push_str("is terminal. No qualified call is allowed on this value.\n");
            }
            ModelExtended::Recursive => text.push_str("has a recursive model.\n"),
            ModelExtended::IsEmpty => text.push_str("has an empty model.\n"),
            ModelExtended::Model {
                names,
                types,
                extension,
            } => {
                text.push_str("has model: ");
                for ((name, ty), ext) in names.iter().zip(types.iter()).zip(extension) {
                    text.push_str(format!("{name}: {ty}").as_str());
                    text.push('\n');
                    text.push_str(ext.fmt_verbose_indented(indent + 1).as_str());
                }
            }
        };
        text
    }
}

impl Display for ModelExtended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.fmt_verbose_indented(0);
        write!(f, "{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::Parser;

    fn client_class() -> anyhow::Result<Class> {
        let src = "
    note
        model: nested
    class NEW_CLIENT
    feature
        x: INTEGER
        nested: NEW_INTEGER
    end
    ";
        let mut parser = Parser::new();
        parser.class_from_source(src)
    }

    fn client_class_2() -> anyhow::Result<Class> {
        let src = "
    note
        model: x
    class NEW_CLIENT_2
    feature
        x: INTEGER
        nested: NEW_INTEGER
    end
    ";
        let mut parser = Parser::new();
        parser.class_from_source(src)
    }

    fn supplier_class() -> anyhow::Result<Class> {
        let src = "
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
        let mut parser = Parser::new();
        parser.class_from_source(src)
    }

    #[test]
    fn extended_model() -> anyhow::Result<()> {
        let system_classes = vec![client_class()?, supplier_class()?];
        let client = &system_classes[0];

        let top_model = client.local_model().clone().extended(&system_classes);

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

        assert_eq!(extension.len(), 1);

        assert_eq!(
            extension[0],
            ModelExtended::Terminal,
            "The class INTEGER is a terminal class for the model.\nFound model of INTEGER: {:#?}",
            extension,
        );

        Ok(())
    }

    #[test]
    fn extended_model2() -> anyhow::Result<()> {
        let system_classes = vec![client_class_2()?, supplier_class()?];

        let client = &system_classes[0];

        let ModelExtended::Model {
            names,
            types,
            extension,
        } = client.local_model().clone().extended(&system_classes)
        else {
            panic!("client must have a populated model.")
        };

        eprintln!(
            "client extended model: {}",
            client.local_model().clone().extended(&system_classes)
        );
        assert_eq!(extension.len(), 1,);
        assert_eq!(extension.first().unwrap(), &ModelExtended::Terminal);
        assert_eq!(names.len(), 1);
        assert_eq!(names.first().unwrap(), "x");

        assert_eq!(types.len(), 1);
        assert_eq!(types.first().unwrap().class_name().unwrap(), "INTEGER");

        Ok(())
    }

    #[test]
    fn display_extended_model() -> anyhow::Result<()> {
        let system_classes = vec![client_class()?, supplier_class()?];
        let client = &system_classes[0];

        let model = client.model_extended(&system_classes);
        assert_eq!(
            format!("{model}"),
            "has model: nested: NEW_INTEGER\n\thas model: value: INTEGER\n\t\tis terminal. No qualified call is allowed on this value.\n"
        );
        Ok(())
    }
}
