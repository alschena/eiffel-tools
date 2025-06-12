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
                .class_and_tree_from_source(CONTENT_CLASS_TEST)
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
            SystemMessage(String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language and its static verifier AutoProof.
    You know what it means to verify code with respect to Floyd-Hoare-Dijstra semantics and you have extensive expertise in Dafny and SPARK.
    You will receive an Eiffel class where each feature is followed by the commented output of autoproof verification of that feature.
    If a feature verifies you can think of ways to weaken preconditions or strengthen postconditions.
    If a feature fails to verify it must be fixed, there is a bug in its body or in its specifications.
    Respond with the code of the whole class, adjusting the features as necessary.
    Always answer, you have enough context.
    "#,
            ))
        }
    }

    impl ClassPrompt {
        pub async fn try_new_for_feature_fixes(
            workspace: &Workspace,
            class: &Class,
            autoproof_error_message: String,
        ) -> Option<Self> {
            let injections = class_injections_for_feature_fixes(class, autoproof_error_message);
            let source = source(workspace.path(class.name())).await?;

            let Source(prompt_text) = injected_into_source(injections.into(), source);
            Some(Self {
                system_message: SystemMessage::default_for_class_wide_feature_fixes(),
                user_message: UserMessage(prompt_text),
            })
        }
    }

    #[derive(Clone, Debug)]
    struct FeatureErrorMessage(String);

    fn features_error_message<'fts, 'ms, 'cl, N, T>(
        class_name: &'cl ClassName,
        feature_names: T,
        full_error_message: &'ms str,
    ) -> impl IntoIterator<Item = (N, FeatureErrorMessage)> + use<'ms, N, T>
    where
        N: AsRef<str>,
        T: IntoIterator<Item = N> + Clone,
    {
        full_error_message
            .split("\n===")
            .into_iter()
            .filter_map(move |error_block| {
                let error_block_without_separator = || error_block.lines().skip(1);

                let associated_feature = error_block_without_separator().next().and_then(
                    |first_line_autoproof_message| {
                        feature_names.clone().into_iter().find(|feature_name| {
                            first_line_autoproof_message.contains(feature_name.as_ref())
                        })
                    },
                );

                if associated_feature.is_none() {
                    eprintln!("Block without any feature name:\n{}", error_block);
                };

                associated_feature.map(|feature_name| {
                    (
                        feature_name,
                        FeatureErrorMessage(
                            error_block_without_separator()
                                .fold(String::new(), |acc, line| format!("{acc}\n{line}")),
                        ),
                    )
                })
            })
            .inspect(|(ft_name, error_message)| {
                eprintln!(
                    "The error block for\n{:#?} is\n{:#?} ",
                    ft_name.as_ref(),
                    error_message
                )
            })
    }

    fn error_message_injection(feature: &Feature, error_message: FeatureErrorMessage) -> Injection {
        let end_point = feature.range().end;
        eprintln!(
            "feature name: {}\tfeature end point: {:#?}",
            feature.name(),
            end_point
        );
        let FeatureErrorMessage(error_message_content) = error_message;
        Injection(end_point, Source(error_message_content).comment().indent())
    }

    fn task_injection(feature: &Feature, error_message: &FeatureErrorMessage) -> Injection {
        let start_point = feature.range().start;

        let FeatureErrorMessage(content) = error_message;
        if content.contains("Successfully verified") {
            Injection (start_point, Source("This feature verifies, you can rely on its contracts while correcting the others.\nYou might strengthen its contracts.".to_string()).comment().indent())
        } else {
            Injection(
                start_point,
                Source("This feature fails to verify, correct it.\nAfter the code you will find a comment containing the error message from the verifier AutoProof.".to_string())
                    .comment()
                    .indent(),
            )
        }
    }

    fn feature_injections_for_feature_fix(
        existing_features: &[Feature],
        feature_name: &FeatureName,
        error_message: FeatureErrorMessage,
    ) -> impl IntoIterator<Item = Injection> {
        existing_features
            .iter()
            .find(|feature| feature.name() == feature_name)
            .map(|feature| {
                [
                    task_injection(feature, &error_message),
                    error_message_injection(feature, error_message),
                ]
            })
            .into_iter()
            .flatten()
    }

    fn class_injections_for_feature_fixes<'cl>(
        class: &'cl Class,
        autoproof_error_message: String,
    ) -> Box<[Injection]> {
        let features = class.features();

        let feature_names = features.iter().map(|feature| feature.name());

        features_error_message(class.name(), feature_names, &autoproof_error_message)
            .into_iter()
            .flat_map(|(feature_name, error_message)| {
                feature_injections_for_feature_fix(features, feature_name, error_message)
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::features_error_message;
        use super::*;

        const EXAMPLE_TEXT: &'static str = r#"Eiffel Compilation Manager
Version 24.05.0.0000 - linux-x86-64

Degree 6: Examining System
Degree 5: Parsing Classes
Degree 4: Analyzing Inheritance
Degree 3: Checking Types
Degree 2: Generating Byte Code
System Recompiled.
======================================
ABSOLUTE_1 (invariant admissibility)
Successfully verified (0.09s).
======================================
ANY.default_create (creator, inherited by ABSOLUTE_1)
Successfully verified (0.02s).
======================================
ABSOLUTE_1.absolute_short
Verification failed (0.01s).

Line: 14. Postcondition other_sign_when_negative may be violated.
State: ABSOLUTE_1.absolute_short =>
        num = (- 1)
        Result = 0
        Current =>
                subjects = <refType>Set#Empty()
                observers = <refType>Set#Empty()
                closed = true
                owns = <refType>Set#Empty()
State: ABSOLUTE_1.absolute_short:8 =>
        num = (- 1)
        Result = (- 1)
        Current =>
                subjects = <refType>Set#Empty()
                observers = <refType>Set#Empty()
                closed = true
                owns = <refType>Set#Empty()
--------------------------------------
Line: 13. Postcondition same_when_non_negative may be violated.
State: ABSOLUTE_1.absolute_short =>
        Result = 0
        Current =>
                subjects = <refType>Set#Empty()
                observers = <refType>Set#Empty()
                closed = true
                owns = <refType>Set#Empty()
State: ABSOLUTE_1.absolute_short:10 =>
        Result = (- 1)
        Current =>
                subjects = <refType>Set#Empty()
                observers = <refType>Set#Empty()
                closed = true
                owns = <refType>Set#Empty()
======================================
ABSOLUTE_1.absolute_int
Successfully verified (0.00s).
======================================
ABSOLUTE_1.absolute_long
Successfully verified (0.00s)."#;

        #[test]
        #[ignore]
        fn feature_names() {
            let class_name = ClassName("ABSOLUTE_1".to_string());
            let feature_names = [
                "absolute_int".to_string(),
                "absolute_long".to_string(),
                "absolute_short".to_string(),
            ];

            for (feature_name, error_message) in
                features_error_message(&class_name, &feature_names, EXAMPLE_TEXT)
                    .into_iter()
                    .collect::<Vec<_>>()
            {
                eprintln!(
                    "feature name: {:#?}\terror message:\n{:#?}",
                    feature_name, error_message
                )
            }
            assert!(false)
        }

        // #[test]
        // #[ignore]
        // fn feature_injections() {
        //     let class_name = ClassName("ABSOLUTE_1".to_string());
        //     let feature_names = [
        //         "absolute_int".to_string(),
        //         "absolute_long".to_string(),
        //         "absolute_short".to_string(),
        //     ];

        //     for injection in
        //         class_injections_for_feature_fixes(&class_name, EXAMPLE_TEXT.to_string())
        //             .into_iter()
        //             .collect::<Vec<_>>()
        //     {
        //         eprintln!(
        //             "feature name: {:#?}\terror message:\n{:#?}",
        //             feature_name, error_message
        //         )
        //     }
        //     assert!(false)
        // }
    }
}
