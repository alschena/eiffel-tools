use super::feature_prompt::model_based_contracts::{
    format_hole_postcondition, format_hole_precondition,
};
use super::*;
use crate::generators::constructor_api;
use crate::workspace::Workspace;
use std::fmt::Debug;
use std::path::Path;
use tracing::warn;

pub struct ClassPrompt {
    system_message: SystemMessage,
    user_message: UserMessage,
}

impl From<ClassPrompt> for Vec<constructor_api::MessageOut> {
    fn from(value: ClassPrompt) -> Self {
        let ClassPrompt {
            system_message,
            user_message,
        } = value;
        vec![system_message.into(), user_message.into()]
    }
}

async fn source<P: AsRef<Path> + Debug>(path: P) -> Option<Source> {
    let raw_text = tokio::fs::read(path)
        .await
        .inspect_err(|e| {
            warn!(
                "[PROMPT FOR QUERIES ON CLASS] fails to read from file for class prompt generation with error {:#?}",
                e
            )
        })
        .ok()?;

    let text = String::from_utf8(raw_text)
        .inspect_err(|e| {
            warn!(
                "[PROMPT FOR QUERIES ON CLASS] fails to convert content of file to UFT-8 with error {:#?}",
                 e
            )
        })
        .ok()?;

    Some(Source(text))
}

fn feature_available_identifiers_injections(
    workspace: &Workspace,
    class_name: &ClassName,
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
    let mut feature_start = feature.range().start;
    feature_start.reset_column();

    [
        Injection(
            feature_start,
            Source::format_model_of_class(workspace, class_name)
                .prepend_if_nonempty("Model of the current class both immediate and inherited: ")
                .comment()
                .indent(),
        ),
        Injection(
            feature_start,
            Source::format_model_of_parameters(workspace, feature.parameters())
                .comment()
                .indent(),
        ),
        Injection(
            feature_start,
            Source::format_available_identifiers_in_feature_preconditon(
                workspace, class_name, feature,
            )
            .comment()
            .indent(),
        ),
        Injection(
            feature_start,
            Source::format_available_identifiers_in_feature_postconditions(
                workspace, class_name, feature,
            )
            .comment()
            .indent(),
        ),
    ]
}

mod class_wide_model_based_contracts {
    use super::*;
    impl SystemMessage {
        fn default_for_class_wide_model_based_contracts() -> Self {
            SystemMessage(String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive a prompt with an Eiffel class; before every feature you will find a comment with extra information about the feature which comes from a project analysis.
In all the precondition and postcondition blocks, you will find a hole in the form <ADD_PRECONDITION_CLAUSES> or <ADD_POSTCONDITION_CLAUSES>.
Substitute in the class all the holes with model-based contracts.
In Eiffel, contracts are model-based when, for all non terminal classes, all qualified calls in all contract clauses are defined in the model of the target and all unqualified calls in all contract clauses are defined in the model of the current class or its ancestors.
For terminal classes, all their features are axiomatically defined; you already have the documents containing the eiffel files and their theory files in boogie.
Answer always, you have sufficient context.
Respond with the same code, substituting the holes with valid eiffel code."#,
            ))
        }
    }
    impl ClassPrompt {
        pub async fn try_new_for_model_based_contracts(
            workspace: &Workspace,
            class: &Class,
        ) -> Option<Self> {
            let injections = class_injections_for_model_based_contracts(workspace, class)
                .into_iter()
                .collect();
            let source = source(workspace.path(class.name())).await?;

            Some(Self {
                system_message: SystemMessage::default_for_class_wide_model_based_contracts(),
                user_message: injected_into_source(injections, source).into(),
            })
        }
    }

    fn feature_injections_for_model_based_contracts(
        workspace: &Workspace,
        class_name: &ClassName,
        feature: &Feature,
    ) -> impl IntoIterator<Item = Injection> {
        feature_available_identifiers_injections(workspace, class_name, feature)
            .into_iter()
            .chain(immediate_feature_contracts_injections(feature))
    }

    fn immediate_feature_contracts_injections(
        feature: &Feature,
    ) -> impl IntoIterator<Item = Injection> {
        feature
            .point_end_preconditions()
            .map(|point| Injection(point, format_hole_precondition(feature)))
            .into_iter()
            .chain(
                feature
                    .point_end_postconditions()
                    .map(|point| Injection(point, format_hole_postcondition(feature))),
            )
    }

    fn class_injections_for_model_based_contracts(
        workspace: &Workspace,
        class: &Class,
    ) -> impl IntoIterator<Item = Injection> {
        class.features().into_iter().flat_map(|feature| {
            feature_injections_for_model_based_contracts(workspace, class.name(), feature)
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::parser::Parser;
        use crate::workspace::Workspace;
        use assert_fs::TempDir;
        use std::path::PathBuf;

        const CONTENT_CLASS_TEST: &'static str = r#"class
	TEST_CLASS
feature
	sum (x,y: INTEGER): INTEGER
		deferred
		end

	product (x,y: INTEGER): INTEGER
		deferred
		end
end
"#;

        fn test_workspace() -> (Workspace, PathBuf) {
            let mut parser = Parser::new();
            let (class, tree) = parser
                .processed_file(CONTENT_CLASS_TEST)
                .expect("fails to construct test class.");
            let mut workspace = Workspace::mock();
            let temp_dir = TempDir::new().expect("fails to create temp dir.");
            workspace.add_file((class.clone(), temp_dir.path().to_path_buf(), tree));
            (workspace, temp_dir.to_path_buf())
        }

        #[test]
        #[ignore]
        fn injected_source_for_class_wide_model_based_contracts() {
            let (workspace, path) = test_workspace();
            let class = workspace
                .class(&path)
                .expect("fails to retrive class in example.");

            let injections = class_injections_for_model_based_contracts(&workspace, class);
            let source = Source(CONTENT_CLASS_TEST.to_string());

            eprintln!("Initial source: {}", &source);

            let injected_source = injected_into_source(injections.into_iter().collect(), source);

            eprintln!("Injected source : {}", injected_source);

            assert!(false)
        }
    }
}

mod class_wide_feature_fixes {
    use super::*;

    impl SystemMessage {
        fn default_for_class_wide_feature_fixes() -> Self {
            SystemMessage(String::new())
        }
    }

    impl ClassPrompt {
        pub async fn try_new_for_feature_fixes(
            workspace: &Workspace,
            class: &Class,
        ) -> Option<Self> {
            let injections = class_injections_for_feature_fixes(workspace, class);
            let source = source(workspace.path(class.name())).await?;

            let Source(prompt_text) =
                injected_into_source(injections.into_iter().collect(), source);
            Some(Self {
                system_message: SystemMessage::default_for_class_wide_feature_fixes(),
                user_message: UserMessage(prompt_text),
            })
        }
    }

    fn feature_injections_for_feature_fixes(
        workspace: &Workspace,
        class_name: &ClassName,
        feature: &Feature,
    ) -> impl IntoIterator<Item = Injection> {
        vec![todo!()]
    }

    fn class_injections_for_feature_fixes(
        workspace: &Workspace,
        class: &Class,
    ) -> impl IntoIterator<Item = Injection> {
        vec![todo!()]
    }
}
