use crate::lib::code_entities::prelude::*;
use anyhow::Context;
use std::cmp::Ordering;
use std::path::Path;
use tracing::info;

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
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code.
"#,
            )),
            source: String::new(),
            injections: Vec::new(),
        }
    }
}

impl Prompt {
    pub async fn for_feature_specification(
        feature: &Feature,
        class_model: &ClassModel,
        filepath: &Path,
        system_classes: &[Class],
    ) -> anyhow::Result<Self> {
        let mut var = Self::default();
        var.set_feature_src(feature, filepath).await?;
        var.add_contracts_injection(feature)?;
        var.add_current_model_injections(class_model);
        var.add_parameters_model_injections(feature, system_classes);
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

    fn offset_precondition(feature: &Feature) -> anyhow::Result<Point> {
        let feature_point = feature.range().start;
        let point_insert_preconditions = feature
            .point_end_preconditions()
            .with_context(|| "The feature:\t{feature:#?} cannot have contracts.")?;
        Ok(point_insert_preconditions - feature_point)
    }

    fn offset_postcondition(feature: &Feature) -> anyhow::Result<Point> {
        let feature_point = feature.range().start;
        let point_insert_postconditions = feature
            .point_end_postconditions()
            .with_context(|| "The feature:\t{feature:#?} cannot have contracts.")?;
        Ok(point_insert_postconditions - feature_point)
    }

    fn hole_preconditions(feature: &Feature) -> String {
        if feature.has_precondition() {
            format!(
                "\n{}<ADD_PRECONDITION_CLAUSES>",
                contract::Precondition::indentation_string()
            )
        } else {
            format!(
                "require\n{}<ADD_PRECONDITION_CLAUSES>\n",
                contract::Precondition::indentation_string(),
            )
        }
    }

    fn hole_postconditions(feature: &Feature) -> String {
        if feature.has_postcondition() {
            format!(
                "\n{}<ADD_POSTCONDITION_CLAUSES>",
                contract::Postcondition::indentation_string()
            )
        } else {
            format!(
                "require\n{}<ADD_POSTCONDITION_CLAUSES>\n",
                contract::Postcondition::indentation_string(),
            )
        }
    }

    fn add_precondition_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_precondition = Self::offset_precondition(feature)?;
        let hole_preconditions = Self::hole_preconditions(feature);
        self.injections
            .push((point_offset_precondition, hole_preconditions));
        Ok(())
    }

    fn add_postcondition_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_postcondition = Self::offset_postcondition(feature)?;
        let hole_postconditions = Self::hole_postconditions(feature);
        self.injections
            .push((point_offset_postcondition, hole_postconditions));
        Ok(())
    }

    fn add_contracts_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        self.add_precondition_injection(feature)?;
        self.add_postcondition_injection(feature)
    }

    fn eiffel_comment(text: String) -> String {
        text.lines().fold(String::new(), |mut acc, line| {
            if !line.trim_start().is_empty() {
                acc.push_str("-- ");
            }
            acc.push_str(line);
            acc.push('\n');
            acc
        })
    }

    fn fmt_current_model(class_name: &ClassName, system_classes: &[Class]) -> String {
        let Some(model) = class_name.inhereted_model(system_classes) else {
            return String::new();
        };
        if model.is_empty() {
            return String::new();
        };
        format!("{model}")
    }

    fn fmt_parameters(feature: &Feature) -> String {
        let parameters = feature.parameters();
        if parameters.is_empty() {
            return String::new();
        }
        format!("{parameters}")
    }

    fn fmt_prestate(s: String) -> String {
        s.lines().fold(String::new(), |mut acc, line| {
            acc.push_str("old ");
            acc.push_str(line);
            acc.push('\n');
            acc
        })
    }

    fn fmt_inline_and_append(s: String) -> String {
        if s.is_empty() {
            s
        } else {
            format!(", {}", s.trim_end().replace("\n", ", "))
        }
    }

    fn fmt_list_possible_preconditions_identifiers(
        &self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<String> {
        let fmt_current_model =
            Self::fmt_inline_and_append(Self::fmt_current_model(class_name, system_classes));
        let fmt_parameters = Self::fmt_inline_and_append(Self::fmt_parameters(feature));

        Ok(format!("Identifiers available in the pre-state for the preconditions: Current: {class_name}{fmt_current_model}{fmt_parameters}.\n"))
    }

    fn add_list_possible_preconditions_identifiers(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<()> {
        let injection_point = Point { row: 0, column: 0 };
        let text =
            self.fmt_list_possible_preconditions_identifiers(class_name, feature, system_classes)?;
        let commented_text = Prompt::eiffel_comment(text);
        self.injections.push((injection_point, commented_text));
        Ok(())
    }

    fn fmt_list_possible_postconditions_identifiers(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<String> {
        let fmt_current_model_prestate = Self::fmt_inline_and_append(Self::fmt_prestate(
            Self::fmt_current_model(class_name, system_classes),
        ));
        let fmt_current_model =
            Self::fmt_inline_and_append(Self::fmt_current_model(class_name, system_classes));
        let fmt_parameters_prestate =
            Self::fmt_inline_and_append(Self::fmt_prestate(Self::fmt_parameters(feature)));
        let fmt_parameters_model = Self::fmt_inline_and_append(Self::fmt_parameters(feature));
        let fmt_return_type = feature
            .return_type()
            .map_or_else(|| String::new(), |ty| format!(", Result: {ty}"));

        Ok(format!("Identifiers available in the pre-state for the postcondition: Current: {class_name}{fmt_current_model_prestate}{fmt_parameters_prestate}.\nIdentifiers available in the post-state for the postcondition: Current: {class_name}{fmt_current_model}{fmt_parameters_model}{fmt_return_type}."))
    }

    fn add_list_possible_postconditions_identifiers(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<()> {
        let injection_point = Point { row: 0, column: 0 };
        let text =
            self.fmt_list_possible_postconditions_identifiers(class_name, feature, system_classes)?;
        let commented_text = Prompt::eiffel_comment(text);
        self.injections.push((injection_point, commented_text));
        Ok(())
    }

    fn add_current_model_injections(&mut self, class_model: &ClassModel) {
        let injection_point = Point { row: 0, column: 0 };
        let model_fmt = format!(
            "For the current class and its ancestors, {}",
            class_model.fmt_verbose_indented(0),
        );
        let display_model_as_comment = Self::eiffel_comment(model_fmt);
        self.injections
            .push((injection_point, display_model_as_comment))
    }

    fn add_parameters_model_injections(&mut self, feature: &Feature, system_classes: &[Class]) {
        let injection_point = Point { row: 0, column: 0 };
        let parameters_fmt = feature.parameters().fmt_model(system_classes);

        let display_parameters_as_comment = Self::eiffel_comment(parameters_fmt);
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
    fn inject_into_source(mut injections: Vec<(Point, String)>, source: String) -> String {
        Self::sort_injections(&mut injections);

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
}

impl From<Prompt> for Vec<super::constructor_api::MessageOut> {
    fn from(value: Prompt) -> Self {
        let text = Prompt::inject_into_source(value.injections, value.source);

        let val = vec![
            super::constructor_api::MessageOut::new_system(value.system_message),
            super::constructor_api::MessageOut::new_user(text),
        ];
        info!("{val:#?}");
        val
    }
}

#[cfg(test)]
mod tests {
    use super::super::constructor_api::MessageOut;
    use super::*;
    use crate::lib::processed_file::ProcessedFile;
    use crate::lib::tree_sitter_extension::Parse;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};
    use async_lsp::lsp_types;

    fn parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        parser
    }

    const SRC_CLIENT_NEW_INTEGER: &'static str = r#"
class A feature
  x (arg: NEW_INTEGER)
    do
    end
end
"#;
    const SRC_NEW_INTEGER: &'static str = r#"note
    	model: value
    class
    	NEW_INTEGER
    feature
    	value: INTEGER
	smaller (other: NEW_INTEGER): BOOLEAN
		do
			Result := value < other.value
		end
    end
        "#;

    const SRC_NEW_INTEGER_SMALLER: &'static str = r#"smaller (other: NEW_INTEGER): BOOLEAN
		do
			Result := value < other.value
		end
"#;

    #[tokio::test]
    async fn set_feature_src_from_file() -> anyhow::Result<()> {
        let mut parser = parser();
        let temp_dir = TempDir::new()?;
        let file = temp_dir.child("test_prompt.e");
        let src = SRC_NEW_INTEGER;
        file.write_str(src)?;

        assert!(file.exists());

        let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("processed file must be produced.");

        let client = processed_file.class();
        let feature = client
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .expect("find feature `smaller`");

        let mut prompt = Prompt::default();
        prompt
            .set_feature_src(feature, processed_file.path())
            .await?;

        assert_eq!(prompt.source, String::from(SRC_NEW_INTEGER_SMALLER));
        Ok(())
    }

    #[tokio::test]
    async fn prompt_boxed_integer_arg() -> anyhow::Result<()> {
        let class = Class::parse(SRC_NEW_INTEGER)?;

        let system_classes = vec![class.clone()];

        let class_model = class.name().model_extended(&system_classes);
        let feature = class
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .with_context(|| "first features is `x`")?;
        let feature_src = SRC_NEW_INTEGER_SMALLER;

        let mut prompt = Prompt::default();
        prompt.set_source(feature_src);
        prompt.add_contracts_injection(feature)?;
        prompt.add_current_model_injections(&class_model);
        prompt.add_parameters_model_injections(feature, &system_classes);

        let messages: Vec<MessageOut> = prompt.clone().into();
        eprintln!("{messages:#?}");

        let system_message = MessageOut::new_system(prompt.system_message);
        let inj = prompt.injections;
        let src = prompt.source;
        let user_message = MessageOut::new_user(Prompt::inject_into_source(inj, src));

        assert_eq!(messages, vec![system_message, user_message]);
        Ok(())
    }

    #[test]
    fn prompt_lists_identifiers() -> anyhow::Result<()> {
        let class = Class::parse(&SRC_NEW_INTEGER)?;
        let class_name = class.name().clone();
        let feature = class
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .with_context(|| "fails to parse feature of `NEW_INTEGER`")?
            .clone();
        eprintln!("{class:#?}");
        let system_classes = vec![class];

        let mut prompt = Prompt::default();
        prompt.set_source(SRC_NEW_INTEGER_SMALLER);
        eprintln!("{prompt:#?}");
        prompt.add_list_possible_preconditions_identifiers(
            &class_name,
            &feature,
            &system_classes,
        )?;
        eprintln!("{prompt:#?}");
        prompt.add_list_possible_postconditions_identifiers(
            &class_name,
            &feature,
            &system_classes,
        )?;
        eprintln!("{prompt:#?}");
        assert_eq!(
            Prompt::inject_into_source(prompt.injections, prompt.source),
            String::from(
                r#"-- Identifiers available in the pre-state for the preconditions: Current: NEW_INTEGER, value: INTEGER, other: NEW_INTEGER.
-- Identifiers available in the pre-state for the postcondition: Current: NEW_INTEGER, old value: INTEGER, old other: NEW_INTEGER.
-- Identifiers available in the post-state for the postcondition: Current: NEW_INTEGER, value: INTEGER, other: NEW_INTEGER, Result: BOOLEAN.
smaller (other: NEW_INTEGER): BOOLEAN
		do
			Result := value < other.value
		end
"#
            )
        );
        Ok(())
    }
}
