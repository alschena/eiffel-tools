use crate::lib::code_entities::class::model::ModelExtended;
use crate::lib::code_entities::prelude::*;
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Parameters {
    pub names: Vec<String>,
    pub types: Vec<EiffelType>,
}
impl Parameters {
    pub fn names(&self) -> &Vec<String> {
        &self.names
    }
    pub fn types(&self) -> &Vec<EiffelType> {
        &self.types
    }
    fn add_parameter(&mut self, id: String, eiffel_type: EiffelType) {
        self.names.push(id);
        self.types.push(eiffel_type);
    }
    pub fn is_empty(&self) -> bool {
        self.names().is_empty() && self.types().is_empty()
    }
    pub fn model_extension<'s, 'system>(
        &'s self,
        system_classes: &'system [Class],
    ) -> impl Iterator<Item = ModelExtended> + use<'s, 'system> {
        self.types()
            .iter()
            .map(|t| t.model_extension(system_classes))
    }
    pub fn fmt_model(&self, system_classes: &[Class]) -> String {
        let parameters_models = self.model_extension(system_classes);

        format!("{self}")
            .lines()
            .zip(parameters_models)
            .map(|(line, model)| format!("The argument {line}\n{}", model.fmt_verbose_indented(1)))
            .collect()
    }
}

impl Display for Parameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let types = self.types();
        let names = self.names();
        let text = names.iter().zip(types.iter()).fold(
            String::new(),
            |mut acc, (parameter_name, parameter_type)| {
                acc.push_str(parameter_name.as_str());
                acc.push(':');
                acc.push(' ');
                acc.push_str(format!("{parameter_type}").as_str());
                acc.push('\n');
                acc
            },
        );
        write!(f, "{text}")?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::Parser;
    use anyhow::Result;

    fn class(source: &str) -> Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(source)
    }

    pub fn integer_parameter(name: String) -> Parameters {
        Parameters {
            names: vec![name],
            types: vec![EiffelType::ClassType(
                "INTEGER".to_string(),
                "INTEGER".to_string(),
            )],
        }
    }

    pub fn new_integer_parameter(name: String) -> Parameters {
        Parameters {
            names: vec![name],
            types: vec![EiffelType::ClassType(
                "NEW_INTEGER".to_string(),
                "NEW_INTEGER".to_string(),
            )],
        }
    }

    #[test]
    fn display_parameter() {
        let p = integer_parameter("test".to_string());
        assert_eq!(format!("{p}"), "test: INTEGER\n");
    }

    #[test]
    fn display_model_parameter() -> anyhow::Result<()> {
        let src = r#"note
	model: value
class
	NEW_INTEGER
feature
	value: INTEGER
end
    "#;
        let system_classes = [class(src)?];
        let p = new_integer_parameter("test".to_string());
        assert_eq!(
            format!("{}", p.fmt_model(&system_classes)),
            "The argument test: NEW_INTEGER\n\thas model: value: INTEGER\n\t\tis terminal. No qualified call is allowed on this value.\n"
        );

        Ok(())
    }
}
