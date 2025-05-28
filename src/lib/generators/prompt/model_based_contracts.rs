use super::*;

impl Prompt {
    fn add_contracts_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        self.add_precondition_injection(feature)?;
        self.add_postcondition_injection(feature)
    }

    fn add_current_model_injections_at_the_beginning(&mut self, class_model: &ClassModel) {
        let model_fmt = format!(
            "For the current class and its ancestors, {}",
            class_model.fmt_verbose_indented(0),
        );
        self.add_commented_injection_at_the_beginning(model_fmt);
    }

    fn add_list_possible_postconditions_identifiers_at_beginning(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<()> {
        let text =
            self.fmt_list_possible_postconditions_identifiers(class_name, feature, system_classes)?;

        self.add_commented_injection_at_the_beginning(text);
        Ok(())
    }

    fn add_list_possible_preconditions_identifiers_before_feature(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<()> {
        let text =
            self.fmt_list_possible_preconditions_identifiers(class_name, feature, system_classes)?;

        self.add_commented_injection_at_the_beginning(text);
        Ok(())
    }

    fn add_parameters_model_injections(&mut self, feature: &Feature, system_classes: &[Class]) {
        let parameters_fmt = feature.parameters().fmt_model(system_classes);

        self.add_commented_injection_at_the_beginning(parameters_fmt);
    }

    fn add_postcondition_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_postcondition = Self::offset_postcondition(feature)?;
        let hole_postconditions = Self::hole_postconditions(feature);
        self.injections
            .push((point_offset_postcondition, hole_postconditions));
        Ok(())
    }

    fn add_precondition_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_precondition = Self::offset_precondition(feature)?;
        let hole_preconditions = Self::hole_preconditions(feature);
        self.injections
            .push((point_offset_precondition, hole_preconditions));
        Ok(())
    }

    pub(super) fn default_for_model_based_contracts() -> Self {
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

    fn fmt_current_model(class_name: &ClassName, system_classes: &[Class]) -> String {
        let Some(model) = class_name.inhereted_model(system_classes) else {
            return String::new();
        };
        if model.is_empty() {
            return String::new();
        };
        format!("{model}")
    }

    fn fmt_inline_and_append(s: String) -> String {
        if s.is_empty() {
            s
        } else {
            format!(", {}", s.trim_end().replace("\n", ", "))
        }
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

    pub async fn feature_specification(
        feature: &Feature,
        class_name: &ClassName,
        class_model: &ClassModel,
        filepath: &Path,
        system_classes: &[Class],
    ) -> anyhow::Result<Self> {
        let mut var = Self::default_for_model_based_contracts();
        var.set_feature_src(feature, filepath).await?;
        var.add_contracts_injection(feature)?;
        var.add_list_possible_preconditions_identifiers_before_feature(
            class_name,
            feature,
            system_classes,
        )?;
        var.add_list_possible_postconditions_identifiers_at_beginning(
            class_name,
            feature,
            system_classes,
        )?;
        var.add_current_model_injections_at_the_beginning(class_model);
        var.add_parameters_model_injections(feature, system_classes);
        Ok(var)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::generators::constructor_api::MessageOut;
    use crate::lib::parser::Parser;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

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

    fn class(source: &str) -> anyhow::Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(source)
    }

    #[tokio::test]
    async fn set_feature_src_from_file() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let temp_dir = TempDir::new()?;
        let file = temp_dir.child("test_prompt.e");
        let src = SRC_NEW_INTEGER;
        file.write_str(src)?;

        let (client, _) = parser.processed_file(SRC_NEW_INTEGER)?;

        let feature = client
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .expect("find feature `smaller`");

        let mut prompt = Prompt::default_for_model_based_contracts();
        prompt.set_feature_src(feature, &file.to_path_buf()).await?;

        assert_eq!(prompt.source, String::from(SRC_NEW_INTEGER_SMALLER));
        Ok(())
    }

    #[tokio::test]
    async fn prompt_boxed_integer_arg() -> anyhow::Result<()> {
        let class = class(SRC_NEW_INTEGER)?;

        let system_classes = vec![class.clone()];

        let class_model = class.name().model_extended(&system_classes);
        let feature = class
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .with_context(|| "first features is `x`")?;
        let feature_src = SRC_NEW_INTEGER_SMALLER;

        let mut prompt = Prompt::default_for_model_based_contracts();
        prompt.set_source(feature_src);
        prompt.add_contracts_injection(feature)?;
        prompt.add_current_model_injections_at_the_beginning(&class_model);
        prompt.add_parameters_model_injections(feature, &system_classes);

        let messages: Vec<MessageOut> = prompt.clone().to_messages();

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
        let class = class(SRC_NEW_INTEGER)?;
        let class_name = class.name().clone();
        let feature = class
            .features()
            .iter()
            .find(|ft| ft.name() == "smaller")
            .with_context(|| "fails to parse feature of `NEW_INTEGER`")?
            .clone();
        eprintln!("{class:#?}");
        let system_classes = vec![class];

        let mut prompt = Prompt::default_for_model_based_contracts();
        prompt.set_source(SRC_NEW_INTEGER_SMALLER);
        eprintln!("{prompt:#?}");
        prompt.add_list_possible_preconditions_identifiers_before_feature(
            &class_name,
            &feature,
            &system_classes,
        )?;
        eprintln!("{prompt:#?}");
        prompt.add_list_possible_postconditions_identifiers_at_beginning(
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
