use super::*;
use crate::generators::constructor_api;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::anyhow;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct FeaturePrompt {
    system_message: SystemMessage,
    user_message: UserMessage,
}

/// Controls which sections are included in the fix-feature user message.
#[derive(Debug, Clone)]
pub struct FixPromptParts {
    /// "The following feature does not verify. Please rewrite it so that it verifies."
    pub task_instruction: bool,
    /// "IMPORTANT: Only modify body/locals. Do NOT modify signature, require, ensure."
    pub modification_constraints: bool,
    pub class_invariant: bool,
    pub precondition_identifiers: bool,
    pub postcondition_identifiers: bool,
    pub error_message: bool,
    /// Include the verbatim feature signature in the output-format instruction.
    /// When false the instruction still asks for ```eiffel + signature, but omits the actual text.
    pub verbatim_signature: bool,
    /// Include a static Eiffel syntax reference for contracts and loops.
    pub syntax_guide: bool,
}

impl Default for FixPromptParts {
    fn default() -> Self {
        Self {
            task_instruction: true,
            modification_constraints: true,
            class_invariant: true,
            precondition_identifiers: true,
            postcondition_identifiers: true,
            error_message: true,
            verbatim_signature: true,
            syntax_guide: true,
        }
    }
}

async fn feature_source(path: &Path, feature: &Feature) -> Option<Source> {
    feature
        .source_unchecked(path)
        .await
        .inspect_err(|e| warn!("fails to read feature source with error: {:#?}", e))
        .ok()
        .map(Source)
}

impl FeaturePrompt {
    /// Get the prompt as a string representation (system + user messages)
    pub fn to_string(&self) -> String {
        format!(
            "System: {}\n\nUser: {}",
            self.system_message.0, self.user_message.0
        )
    }
}

impl From<FeaturePrompt> for Vec<constructor_api::MessageOut> {
    fn from(value: FeaturePrompt) -> Self {
        let FeaturePrompt {
            system_message,
            user_message,
        } = value;
        vec![system_message.into(), user_message.into()]
    }
}

fn feature_model_identifiers_injections(
    workspace: &Workspace,
    class_name: &ClassName,
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
    let beginning = Point { row: 0, column: 0 };

    [
        Injection(
            beginning,
            Source::format_model_of_class(workspace, class_name)
                .prepend_if_nonempty("Model of the current class both immediate and inherited: ")
                .comment()
                .indent(),
        ),
        Injection(
            beginning,
            Source::format_model_of_parameters(workspace, feature.parameters())
                .comment()
                .indent(),
        ),
    ]
}

fn feature_identifiers_injections(
    workspace: &Workspace,
    class_name: &ClassName,
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
    let beginning = Point { row: 0, column: 0 };

    [
        Injection(
            beginning,
            Source::format_available_identifiers_in_feature_preconditon(
                workspace, class_name, feature,
            ),
        ),
        Injection(
            beginning,
            Source::format_available_identifiers_in_feature_postconditions(
                workspace, class_name, feature,
            ),
        ),
    ]
}

mod fix_feature {
    use super::*;

    impl SystemMessage {
        pub fn default_for_feature_fixes(signature: &str, verbatim_signature: bool) -> Self {
            let verbatim_hint = if verbatim_signature {
                format!(" The signature is: {signature}")
            } else {
                String::new()
            };
            SystemMessage(format!(
                "You are a coding assistant, expert in the Eiffel programming language and AutoProof static verifier.\n\
                 You will receive context about a class followed by an Eiffel feature that does not verify, and the AutoProof error.\n\
                 Respond with a corrected version of the feature.\n\
                 IMPORTANT: You must ONLY modify the feature body (the code between 'do' and 'end') and/or the local variable declarations (the 'local' clause).\n\
                 You must NOT modify:\n\
                 - The feature signature (name, parameters, return type)\n\
                 - Preconditions (the 'require' clause)\n\
                 - Postconditions (the 'ensure' clause)\n\
                 Preserve all contracts exactly as they are in the original code.\n\
                 Start your response with a fenced code block. \
                 The first line of your response must be ```eiffel and the second line must be the feature signature.{verbatim_hint}\n\
                 Do not include any explanation or prose before the code block.",
            ))
        }
    }

    fn task_instruction_section() -> String {
        "The following feature does not verify.\nPlease rewrite it so that it verifies.\n"
            .to_string()
    }

    fn modification_constraints_section() -> String {
        "IMPORTANT: Only modify the feature body (code between 'do' and 'end') and/or local \
         variable declarations ('local' clause). Do NOT modify the signature, preconditions \
         ('require'), or postconditions ('ensure').\n"
            .to_string()
    }

    fn class_invariant_section(class: &Class) -> Option<String> {
        let invariant = Source::class_invariant(class);
        if invariant.0.is_empty() {
            return None;
        }
        let mut s = String::from("Class invariant:\n");
        for line in invariant.0.lines() {
            s.push('\t');
            s.push_str(line);
            s.push('\n');
        }
        Some(s)
    }

    fn precondition_identifiers_section(
        workspace: &Workspace,
        class: &Class,
        feature: &Feature,
    ) -> Option<String> {
        let ids = Source::precondition_identifiers_raw(workspace, class.name(), feature);
        (!ids.0.is_empty()).then_some(ids.0)
    }

    fn postcondition_identifiers_section(
        workspace: &Workspace,
        class: &Class,
        feature: &Feature,
    ) -> Option<String> {
        let ids = Source::postcondition_identifiers_raw(workspace, class.name(), feature);
        (!ids.0.is_empty()).then_some(ids.0)
    }

    fn feature_code_section(source: &Source) -> String {
        let mut s = String::from("Feature to fix:\n```eiffel\n");
        s.push_str(&source.0);
        if !source.0.ends_with('\n') {
            s.push('\n');
        }
        s.push_str("```\n");
        s
    }

    fn syntax_guide_section() -> String {
        // Grammar reference:
        //   attribute_or_routine = [notes] [precondition] [local] feature_body [postcondition] [rescue] end
        //   precondition = 'require' ['else'] {assertion_clause}
        //   postcondition = 'ensure' ['then'] {assertion_clause}
        //   assertion_clause = [tag ':'] expression
        //   loop = [across … as id] [from …] [invariant …] [until …] 'loop' … [variant [tag:] expr] end
        //   Only 'loop' and 'end' are mandatory in a loop; all other loop clauses are optional.
        "Eiffel syntax reference:\n\
         \n\
         -- is the comment syntax in Eiffel (everything from -- to end of line is a comment).\n\
         \n\
         -- Feature with precondition and postcondition:\n\
         divide (divisor: INTEGER): INTEGER\n\
         \t\trequire\n\
         \t\t\tdivisor_non_zero: divisor /= 0     -- tag is optional; 'require else' weakens inherited pre\n\
         \t\tlocal\n\
         \t\t\treminder: INTEGER                  -- local clause comes before 'do', after 'require'\n\
         \t\tdo\n\
         \t\t\treminder := value \\\\ divisor\n\
         \t\t\tResult := value // divisor\n\
         \t\tensure\n\
         \t\t\t                                   -- 'ensure then' strengthens inherited post\n\
         \t\t\tresult_definition: Result * divisor <= old value  -- 'old expr' = value of expr at entry\n\
         \t\t\tremainder_bounds: Result >= 0      -- 'Result' is the return value\n\
         \t\tend\n\
         \n\
         -- Feature with full loop (all loop clauses are optional except 'loop … end'):\n\
         sum (n: INTEGER): INTEGER\n\
         \t\trequire\n\
         \t\t\tn_non_negative: n >= 0\n\
         \t\tlocal\n\
         \t\t\ti: INTEGER\n\
         \t\tdo\n\
         \t\t\tfrom\n\
         \t\t\t\ti := 0\n\
         \t\t\t\tResult := 0\n\
         \t\t\tinvariant\n\
         \t\t\t\tbounds: 0 <= i and i <= n        -- holds before loop and after every iteration\n\
         \t\t\t\tpartial: Result = i * (i + 1) // 2\n\
         \t\t\tuntil\n\
         \t\t\t\ti >= n\n\
         \t\t\tloop\n\
         \t\t\t\ti := i + 1\n\
         \t\t\t\tResult := Result + i\n\
         \t\t\tvariant\n\
         \t\t\t\tn - i                            -- [optional tag:] non-negative integer, strictly decreasing\n\
         \t\t\tend\n\
         \t\tensure\n\
         \t\t\tresult_correct: Result = n * (n + 1) // 2\n\
         \t\tend\n"
            .to_string()
    }

    fn error_message_section(error_message: &str) -> Option<String> {
        let cleaned: String = error_message
            .lines()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        (!cleaned.is_empty()).then(|| format!("AutoProof error:\n{cleaned}\n"))
    }

    fn build_user_message(
        workspace: &Workspace,
        class: &Class,
        feature: &Feature,
        source: &Source,
        error_message: String,
        parts: &FixPromptParts,
    ) -> UserMessage {
        let mut msg = String::new();

        if parts.task_instruction {
            msg.push_str(&task_instruction_section());
        }
        if parts.modification_constraints {
            msg.push_str(&modification_constraints_section());
        }

        let context_parts: Vec<String> = [
            parts.class_invariant
                .then(|| class_invariant_section(class))
                .flatten(),
            parts
                .precondition_identifiers
                .then(|| precondition_identifiers_section(workspace, class, feature))
                .flatten(),
            parts
                .postcondition_identifiers
                .then(|| postcondition_identifiers_section(workspace, class, feature))
                .flatten(),
        ]
        .into_iter()
        .flatten()
        .collect();

        if !context_parts.is_empty() {
            msg.push_str("\nContext:\n");
            for part in context_parts {
                msg.push_str(&part);
            }
        }

        if parts.syntax_guide {
            msg.push('\n');
            msg.push_str(&syntax_guide_section());
        }

        msg.push('\n');
        msg.push_str(&feature_code_section(source));

        if parts.error_message {
            if let Some(s) = error_message_section(&error_message) {
                msg.push_str(&s);
            }
        }

        UserMessage(msg)
    }

    impl FeaturePrompt {
        pub async fn try_new_for_feature_fixes(
            workspace: &Workspace,
            filepath: &Path,
            feature_name: &FeatureName,
            error_message: String,
            parts: FixPromptParts,
        ) -> Option<Self> {
            let Some(class) = workspace.class(filepath) else {
                warn!("There is no class at {filepath:#?}");
                return None;
            };
            let Some(feature) = class.features().iter().find(|ft| ft.name() == feature_name) else {
                warn!(
                    "There is no feature called {feature_name} in {}",
                    class.name()
                );
                return None;
            };
            let source = feature_source(filepath, feature).await?;
            let signature = source.0.lines().next().unwrap_or("").trim_end();

            Some(Self {
                system_message: SystemMessage::default_for_feature_fixes(
                    signature,
                    parts.verbatim_signature,
                ),
                user_message: build_user_message(
                    workspace,
                    class,
                    feature,
                    &source,
                    error_message,
                    &parts,
                ),
            })
        }
    }
}

pub mod model_based_contracts {
    use super::*;

    fn injections(
        workspace: &Workspace,
        class_name: &ClassName,
        feature: &Feature,
    ) -> impl IntoIterator<Item = Injection> {
        contract_injections(feature)
            .into_iter()
            .chain(std::iter::once(Injection(
                Point { row: 0, column: 0 },
                Source(
                    r#"Write the model based contracts of the following feature.
Answer always, you have enough context."#
                        .to_string(),
                )
                .comment()
                .indent(),
            )))
            .chain(feature_model_identifiers_injections(
                workspace, class_name, feature,
            ))
            .chain(feature_identifiers_injections(
                workspace, class_name, feature,
            ))
    }

    fn contract_injections(feature: &Feature) -> impl IntoIterator<Item = Injection> {
        offsetted_end_precondition(feature)
            .into_iter()
            .map(|end_pre| Injection(end_pre, format_hole_precondition(feature)))
            .chain(
                offsetted_end_postcondition(feature)
                    .map(|end_post| Injection(end_post, format_hole_postcondition(feature))),
            )
    }

    fn offsetted_end_precondition(feature: &Feature) -> Option<Point> {
        feature
            .point_end_preconditions()
            .map(|end_preconditions| end_preconditions - feature.range().start)
    }

    fn offsetted_end_postcondition(feature: &Feature) -> Option<Point> {
        feature
            .point_end_postconditions()
            .map(|end_preconditions| end_preconditions - feature.range().start)
    }

    pub fn format_hole_precondition(feature: &Feature) -> Source {
        if feature.has_precondition() {
            Source("\n\t\t\t<ADD_PRECONDITION_CLAUSES>".to_string())
        } else {
            Source("require\n\t\t\t<ADD_PRECONDITION_CLAUSES>\n\t\t".to_string())
        }
    }

    pub fn format_hole_postcondition(feature: &Feature) -> Source {
        if feature.has_postcondition() {
            Source("\n\t\t\t<ADD_POSTCONDITION_CLAUSES>".to_string())
        } else {
            Source("ensure\n\t\t\t<ADD_POSTCONDITION_CLAUSES>\n\t\t".to_string())
        }
    }

    impl SystemMessage {
        fn default_for_feature_wide_model_based_contracts() -> Self {
            SystemMessage(String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language and in formal methods.
    You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
    You will receive a prompt in eiffel code with holes of the form <ADD_*>.
    Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
    Answer always, you have sufficient context.
    Respond with the same code, substituting the holes with valid eiffel code.
    "#,
            ))
        }
    }

    impl FeaturePrompt {
        pub async fn try_new_for_feature_specification(
            workspace: &Workspace,
            file: &Path,
            feature: &Feature,
        ) -> anyhow::Result<Self> {
            let class = workspace
                .class(file)
                .ok_or_else(|| anyhow!("fails to find class at {:#?}", file))?;

            let source = feature_source(file, feature).await.with_context(|| {
                format!(
                    "the feature {:#?} does not support the addition of contracts.",
                    feature.name()
                )
            })?;

            let injections = injections(workspace, class.name(), feature)
                .into_iter()
                .collect();

            Ok(Self {
                system_message: SystemMessage::default_for_feature_wide_model_based_contracts(),
                user_message: injected_into_source(injections, source).into(),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::parser::Parser;
        use assert_fs::TempDir;
        use std::path::PathBuf;

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

        fn test_workspace() -> (Workspace, PathBuf) {
            let mut parser = Parser::default();
            let (class, tree) = parser
                .class_and_tree_from_source(SRC_NEW_INTEGER)
                .expect("fails to construct test class.");
            let mut workspace = Workspace::new();
            let temp_dir = TempDir::new().expect("fails to create temp dir.");
            workspace.add_file((class.clone(), temp_dir.to_path_buf(), tree));
            (workspace, temp_dir.to_path_buf())
        }

        #[tokio::test]
        async fn prompt_boxed_integer_arg() {
            let (workspace, path) = test_workspace();

            let class = workspace.class(&path).expect("fails to find class.");

            let feature = class
                .features()
                .iter()
                .find(|ft| ft.name() == "smaller")
                .expect("first feature is `x`");

            let class = workspace.class(&path).expect("fails to find test class.");

            let injections = injections(&workspace, class.name(), feature)
                .into_iter()
                .collect();

            eprintln!(
                "user message in feature prompt:\n{}",
                injected_into_source(
                    injections,
                    Source(SRC_NEW_INTEGER_SMALLER.to_string()).indent()
                )
            );
        }
    }
}
