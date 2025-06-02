use super::*;

impl FeaturePrompt {
    fn add_contracts_injection(&mut self, feature: &Feature) -> anyhow::Result<()> {
        self.add_injection_of_precondition_hole(feature)?;
        self.add_injections_of_postcondition_hole(feature)
    }

    fn add_injection_of_current_model_at_the_beginning(&mut self, class_model: &ClassModel) {
        let model_fmt = format!(
            "Model of the current class and its ancestors: {}",
            class_model.fmt_verbose_indented(0),
        );
        self.add_commented_injection_at_the_beginning(model_fmt);
    }

    fn add_injection_of_list_of_possible_postconditions_identifiers_at_beginning(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<()> {
        let text = Self::format_available_identifiers_in_feature_postconditions(
            class_name,
            feature,
            system_classes,
        )?;

        self.add_commented_injection_at_the_beginning(text);
        Ok(())
    }

    fn add_injection_of_list_of_possible_preconditions_identifiers_at_beginning(
        &mut self,
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) {
        let text = Self::format_available_identifiers_in_feature_preconditon(
            class_name,
            feature,
            system_classes,
        );

        self.add_commented_injection_at_the_beginning(text);
    }

    fn add_injections_of_models_of_parameters(
        &mut self,
        feature: &Feature,
        system_classes: &[Class],
    ) {
        let parameters_fmt = feature.parameters().formatted_model(system_classes);

        self.add_commented_injection_at_the_beginning(parameters_fmt);
    }

    fn add_injections_of_postcondition_hole(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_postcondition = Self::offset_postcondition(feature)?;
        let hole_postconditions = if feature.has_postcondition() {
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
            .push((point_offset_postcondition, hole_postconditions));
        Ok(())
    }

    fn add_injection_of_precondition_hole(&mut self, feature: &Feature) -> anyhow::Result<()> {
        let point_offset_precondition = Self::offset_precondition(feature)?;
        let hole_preconditions = if feature.has_precondition() {
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
Answer always, you have sufficient context.
Respond with the same code, substituting the holes with valid eiffel code.
"#,
            )),
            source: String::new(),
            injections: Vec::new(),
        }
    }

    pub async fn for_feature_specification(
        workspace: &Workspace,
        file: &Path,
        feature: &Feature,
    ) -> anyhow::Result<Self> {
        let class = workspace
            .class(file)
            .ok_or_else(|| anyhow!("fails to find class at {:#?}", file))?;

        let mut prompt = Self::default_for_model_based_contracts();

        prompt.set_feature_src(feature, file).await?;
        prompt.add_contracts_injection(feature)?;
        prompt.add_commented_injection_at_the_beginning(
            r#"Write the model based contracts of the following feature.
            Answer always, you have enough context."#,
        );
        prompt.add_injection_of_list_of_possible_preconditions_identifiers_at_beginning(
            class.name(),
            feature,
            workspace.system_classes(),
        );
        prompt.add_injection_of_list_of_possible_postconditions_identifiers_at_beginning(
            class.name(),
            feature,
            workspace.system_classes(),
        )?;
        prompt.add_injection_of_current_model_at_the_beginning(
            &class.name().model_extended(workspace.system_classes()),
        );
        prompt.add_injections_of_models_of_parameters(feature, workspace.system_classes());

        Ok(prompt)
    }

    fn format_current_model(class_name: &ClassName, system_classes: &[Class]) -> String {
        let Some(model) = class_name.inhereted_model(system_classes) else {
            return String::new();
        };
        if model.is_empty() {
            return String::new();
        };
        format!("{model}")
    }

    fn format_inline_and_append(s: String) -> String {
        if s.is_empty() {
            s
        } else {
            format!(", {}", s.trim_end().replace("\n", ", "))
        }
    }

    fn format_available_identifiers_in_feature_postconditions(
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> anyhow::Result<String> {
        let fmt_current_model_prestate = Self::format_inline_and_append(Self::format_prestate(
            Self::format_current_model(class_name, system_classes),
        ));
        let fmt_current_model =
            Self::format_inline_and_append(Self::format_current_model(class_name, system_classes));
        let fmt_parameters_prestate =
            Self::format_inline_and_append(Self::format_prestate(Self::format_parameters(feature)));
        let fmt_parameters_model = Self::format_inline_and_append(Self::format_parameters(feature));
        let fmt_return_type = feature
            .return_type()
            .map_or_else(String::new, |ty| format!(", Result: {ty}"));

        Ok(format!(
            "Identifiers available in the pre-state for the postcondition: Current: {class_name}{fmt_current_model_prestate}{fmt_parameters_prestate}.\nIdentifiers available in the post-state for the postcondition: Current: {class_name}{fmt_current_model}{fmt_parameters_model}{fmt_return_type}."
        ))
    }

    fn format_available_identifiers_in_feature_preconditon(
        class_name: &ClassName,
        feature: &Feature,
        system_classes: &[Class],
    ) -> String {
        let fmt_current_model =
            Self::format_inline_and_append(Self::format_current_model(class_name, system_classes));
        let fmt_parameters = Self::format_inline_and_append(Self::format_parameters(feature));

        format!(
            "Identifiers available in the pre-state for the preconditions: Current: {class_name}{fmt_current_model}{fmt_parameters}.\n"
        )
    }

    fn format_parameters(feature: &Feature) -> String {
        let parameters = feature.parameters();
        if parameters.is_empty() {
            return String::new();
        }
        format!("{parameters}")
    }

    fn format_prestate(s: String) -> String {
        s.lines().fold(String::new(), |mut acc, line| {
            acc.push_str("old ");
            acc.push_str(line);
            acc.push('\n');
            acc
        })
    }

    fn offset_postcondition(feature: &Feature) -> anyhow::Result<Point> {
        let feature_start = feature.range().start;
        let end_postconditions = feature
            .point_end_postconditions()
            .with_context(|| "The feature:\t{feature:#?} cannot have contracts.")?;
        Ok(end_postconditions - feature_start)
    }

    fn offset_precondition(feature: &Feature) -> anyhow::Result<Point> {
        let feature_start = feature.range().start;
        let end_preconditions = feature
            .point_end_preconditions()
            .with_context(|| "The feature:\t{feature:#?} cannot have contracts.")?;
        Ok(end_preconditions - feature_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::constructor_api::MessageOut;
    use crate::parser::Parser;
    use assert_fs::prelude::*;
    use assert_fs::{TempDir, fixture::FileWriteStr};

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

        let mut prompt = FeaturePrompt::default_for_model_based_contracts();
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

        let mut prompt = FeaturePrompt::default_for_model_based_contracts();
        prompt.set_source(feature_src);
        prompt.add_contracts_injection(feature)?;
        prompt.add_injection_of_current_model_at_the_beginning(&class_model);
        prompt.add_injections_of_models_of_parameters(feature, &system_classes);

        let messages: Vec<MessageOut> = prompt.clone().into_llm_chat_messages();

        eprintln!("{messages:#?}");

        let system_message = MessageOut::new_system(prompt.system_message);
        let inj = prompt.injections;
        let src = prompt.source;
        let user_message = MessageOut::new_user(FeaturePrompt::inject_into_source(inj, src));

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

        let mut prompt = FeaturePrompt::default_for_model_based_contracts();
        prompt.set_source(SRC_NEW_INTEGER_SMALLER);
        eprintln!("{prompt:#?}");
        prompt.add_injection_of_list_of_possible_preconditions_identifiers_at_beginning(
            &class_name,
            &feature,
            &system_classes,
        );
        eprintln!("{prompt:#?}");
        prompt.add_injection_of_list_of_possible_postconditions_identifiers_at_beginning(
            &class_name,
            &feature,
            &system_classes,
        )?;
        eprintln!("{prompt:#?}");
        assert_eq!(
            FeaturePrompt::inject_into_source(prompt.injections, prompt.source),
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
