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

async fn feature_source(path: &Path, feature: &Feature) -> Option<Source> {
    feature
        .source_unchecked(path)
        .await
        .inspect_err(|e| warn!("fails to read feature source with error: {:#?}", e))
        .ok()
        .map(|content| Source(content))
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
            )
            .comment()
            .indent(),
        ),
        Injection(
            beginning,
            Source::format_available_identifiers_in_feature_postconditions(
                workspace, class_name, feature,
            )
            .comment()
            .indent(),
        ),
    ]
}

mod fix_feature {
    use super::*;

    impl SystemMessage {
        pub fn default_for_feature_fixes() -> Self {
            SystemMessage(String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language in writing formally verified code.
    You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
    You will receive an eiffel snippet with a comment identifying the routine to fix which contains an error message of AutoProof.
    Answer always, you have enough context from the system prompt and the user prompt.
    Respond with a correct version of the same routine.
    "#,
            ))
        }
    }

    fn injections(
        workspace: &Workspace,
        class: &Class,
        feature: &Feature,
        error_message: String,
    ) -> impl IntoIterator<Item = Injection> {
        let message = Source(
            error_message
                .lines()
                .filter(|line| !line.is_empty())
                .fold(String::new(), |acc, line| format!("{acc}{line}\n"))
                .trim_end()
                .to_string(),
        );
        let Range { start, end } = feature.range();
        [
            Injection(*end - *start, Source(message.to_string()).comment()),
            Injection(
                Point { row: 0, column: 0 },
                Source("The following feature does not verify.\nPlease, rewrite it such that the class will verify".to_string())
                    .comment(),
            ),
            Injection(
                Point { row: 0, column: 0 },
                Source::class_invariant(class).indent().prepend_if_nonempty("This is the current class' immediate class invariant:\n").comment(),
            ),
        ].into_iter().chain(feature_identifiers_injections(workspace, class.name(), feature))
    }

    impl FeaturePrompt {
        pub async fn try_new_for_feature_fixes(
            workspace: &Workspace,
            filepath: &Path,
            feature: &Feature,
            error_message: String,
        ) -> Option<Self> {
            let Some(class) = workspace.class(filepath) else {
                warn!("There is no class at {filepath:#?}");
                return None;
            };
            let injections = injections(workspace, class, feature, error_message)
                .into_iter()
                .collect();
            let source = feature_source(filepath, feature).await?;

            Some(Self {
                system_message: SystemMessage::default_for_feature_fixes(),
                user_message: injected_into_source(injections, source).into(),
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
            Source(format!("\n\t\t\t<ADD_PRECONDITION_CLAUSES>",))
        } else {
            Source(format!("require\n\t\t\t<ADD_PRECONDITION_CLAUSES>\n\t\t",))
        }
    }

    pub fn format_hole_postcondition(feature: &Feature) -> Source {
        if feature.has_postcondition() {
            Source(format!("\n\t\t\t<ADD_POSTCONDITION_CLAUSES>",))
        } else {
            Source(format!("ensure\n\t\t\t<ADD_POSTCONDITION_CLAUSES>\n\t\t",))
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
            let mut parser = Parser::new();
            let (class, tree) = parser
                .class_and_tree_from_source(SRC_NEW_INTEGER)
                .expect("fails to construct test class.");
            let mut workspace = Workspace::mock();
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
