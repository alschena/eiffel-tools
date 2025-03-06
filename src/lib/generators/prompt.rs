use crate::lib::code_entities::prelude::*;
use anyhow::anyhow;
use std::cmp::Ordering;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Prompt {
    system_message: String,
    source: String,
    /// Pairs to be inserted in the source; the left is the point-offset; the right is the string to insert.
    injections: Vec<(Point, String)>,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            system_message: (String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
You will respond with the same code, substituting the holes with valid eiffel code.
"#,
            )),
            source: String::new(),
            injections: Vec::new(),
        }
    }
}

impl Prompt {
    pub fn for_feature_specification(
        feature: &Feature,
        class_model: &ClassModel,
        filepath: &Path,
        system_classes: &[Class],
    ) -> anyhow::Result<Self> {
        let mut var = Self::default();
        var.set_feature_src(feature, filepath);
        var.add_feature_contracts_injections_for_feature_source(feature);
        var.add_model_of_current_injections(class_model);
        var.add_model_of_parameters_injections(feature, system_classes);
        Ok(var)
    }
    fn set_system_message(&mut self, preable: &str) {
        self.system_message.clear();
        self.system_message.push_str(preable);
    }
    fn set_source(&mut self, source: &str) {
        self.source.clear();
        self.source.push_str(source);
    }
    fn set_injections(&mut self, injections: &mut Vec<(Point, String)>) {
        self.injections.clear();
        self.injections.append(injections);
    }
    async fn set_feature_src(&mut self, feature: &Feature, filepath: &Path) -> anyhow::Result<()> {
        let feature_src = feature.src_unchecked(filepath).await?;
        self.source.clear();
        self.source.push_str(&feature_src);
        Ok(())
    }
    fn add_feature_contracts_injections_for_feature_source(
        &mut self,
        feature: &Feature,
    ) -> anyhow::Result<()> {
        let feature_point = feature.range().start().clone();
        let Some(point_insert_preconditions) = feature.point_end_preconditions() else {
            return Err(anyhow!(
                "Only attributes with an attribute block and routines support adding preconditions"
            ));
        };
        let diff_point_insert_preconditions =
            point_insert_preconditions.clone() - feature_point.clone();
        let Some(point_insert_postconditions) = feature.point_end_postconditions() else {
            return Err(anyhow!("Only attributes with an attribute block and routines support adding postconditions"));
        };
        let diff_point_insert_postconditions = point_insert_postconditions.clone() - feature_point;
        let precondition_hole = if feature.has_precondition() {
            format!(
                "\n{}<ADD_PRECONDITION_CLAUSES>",
                contract::Precondition::indentation_string()
            )
        } else {
            format!(
                "require\n{}<ADD_PRECONDITION_CLAUSES>\n",
                contract::Precondition::indentation_string(),
            )
        };
        let postcondition_hole = if feature.has_postcondition() {
            format!(
                "\n{}<ADD_POSTCONDITION_CLAUSES>",
                contract::Postcondition::indentation_string()
            )
        } else {
            format!(
                "ensure\n{}<ADD_POSTCONDITION_CLAUSES>\n",
                contract::Postcondition::indentation_string(),
            )
        };
        self.injections
            .push((diff_point_insert_preconditions, precondition_hole));
        self.injections
            .push((diff_point_insert_postconditions, postcondition_hole));
        Ok(())
    }
    fn add_model_of_current_injections(&mut self, class_model: &ClassModel) {
        let injection_point = Point { row: 0, column: 0 };
        let display_model = format!(
            "For the current class and its ancestors, {}",
            class_model.fmt_indented(0),
        );
        let display_model_as_comment =
            display_model.lines().fold(String::new(), |mut acc, line| {
                if !line.trim_start().is_empty() {
                    acc.push_str("-- ");
                }
                acc.push_str(line);
                acc.push('\n');
                acc
            });
        self.injections
            .push((injection_point, display_model_as_comment))
    }
    fn add_model_of_parameters_injections(&mut self, feature: &Feature, system_classes: &[Class]) {
        let injection_point = Point { row: 0, column: 0 };
        let parameters = feature.parameters();
        let parameters_types = parameters.types();
        let parameters_names = parameters.names();

        let mut parameters_models = parameters_types
            .iter()
            .map(|t| t.model_extension(system_classes));
        let mut parameters_types = parameters_types.iter();
        let mut parameters_names = parameters_names.iter();

        let mut display_parameters = String::new();
        while let (Some(n), Some(ty), Some(m)) = (
            parameters_names.next(),
            parameters_types.next(),
            parameters_models.next(),
        ) {
            display_parameters
                .push_str(format!("For the argument {n}: {ty}\n{}", m.fmt_indented(1)).as_str());
        }

        let display_parameters_as_comment =
            display_parameters
                .lines()
                .fold(String::new(), |mut acc, line| {
                    if !line.trim_start().is_empty() {
                        acc.push_str("-- ");
                    }
                    acc.push_str(line);
                    acc.push('\n');
                    acc
                });
        self.injections
            .push((injection_point, display_parameters_as_comment));
    }
}

impl Prompt {
    fn sort_injections(injections: &mut [(Point, String)]) {
        injections.sort_by(
            |(Point { row, column }, _),
             (
                Point {
                    row: rhs_r,
                    column: rhs_c,
                },
                _,
            )| {
                let val = row.cmp(rhs_r);
                if val == Ordering::Equal {
                    column.cmp(rhs_c)
                } else {
                    val
                }
            },
        );
    }
    fn inject_sorted_to_source(injections: Vec<(Point, String)>, source: String) -> String {
        let mut text = String::new();
        for (linenum, line) in source.lines().enumerate() {
            // Select injections of current line;
            // Relies on ordering of injections;
            let mut current_injections =
                injections
                    .iter()
                    .filter_map(|&(Point { row, column }, ref text)| {
                        (row == linenum).then_some((column, text))
                    });
            // If there are no injections, add line to the text.
            let Some((mut oc, oi)) = current_injections.next() else {
                text.push_str(line);
                text.push('\n');
                continue;
            };
            text.push_str(&line[..oc]);
            text.push_str(oi);
            for (nc, ni) in current_injections {
                text.push_str(&line[oc..nc]);
                text.push_str(ni);
                oc = nc;
            }
            text.push_str(&line[oc..]);
            text.push('\n');
        }
        text
    }
    pub fn to_llm_messages(self) -> Vec<super::constructor_api::MessageOut> {
        let system_message = self.system_message;
        let source = self.source;

        let mut injections = self.injections;
        Self::sort_injections(&mut injections);

        let text = Self::inject_sorted_to_source(injections, source);
        vec![
            super::constructor_api::MessageOut::new_system(system_message),
            super::constructor_api::MessageOut::new_user(text),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::constructor_api::MessageOut;
    use super::*;
    use crate::lib::processed_file::ProcessedFile;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    fn parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        parser
    }

    #[tokio::test]
    async fn prompt_boxed_integer_arg() -> anyhow::Result<()> {
        let mut parser = parser();
        let temp_dir = TempDir::new()?;
        let src = r#"
class A feature
  x (arg: NEW_INTEGER)
    do
    end
end
        "#;
        let file = temp_dir.child("test_prompt.e");
        file.write_str(src)?;

        assert!(file.exists());

        let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("processed file must be produced.");

        let class = processed_file.class();
        let src_supplier = r#"note
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
    "#;
        let supplier = Class::from_source(&src_supplier);

        let system_classes = vec![class.clone(), supplier.clone()];
        let class_model = class
            .name()
            .model_extended(&system_classes)
            .unwrap_or_default();

        let feature = class.features().first().expect("first features is `x`");

        let mut prompt = Prompt::default();
        prompt
            .set_feature_src(feature, processed_file.path())
            .await?;
        prompt.add_feature_contracts_injections_for_feature_source(feature)?;
        prompt.add_model_of_current_injections(&class_model);
        prompt.add_model_of_parameters_injections(feature, &system_classes);

        let messages = prompt.clone().to_llm_messages();
        eprintln!("{messages:#?}");

        let system_message = MessageOut::new_system(prompt.system_message);
        let inj = prompt.injections;
        let src = prompt.source;
        let user_message = MessageOut::new_user(Prompt::inject_sorted_to_source(inj, src));

        assert!(messages.contains(&system_message));
        assert!(messages.contains(&user_message));
        Ok(())
    }
}
