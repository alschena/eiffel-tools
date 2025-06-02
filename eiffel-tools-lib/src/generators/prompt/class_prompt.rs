use crate::code_entities::prelude::*;
use crate::eiffel_source::Indent;
use crate::workspace::Workspace;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;
use std::path::Path;
use tracing::warn;

struct Source(String);

impl Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Source {
    fn comment(&self) -> Self {
        let Source(text) = self;

        Source(text.lines().fold(String::new(), |mut acc, line| {
            if !line.trim_start().is_empty() {
                acc.push_str("-- ");
            }
            acc.push_str(line);
            acc.push('\n');
            acc
        }))
    }

    fn format_available_identifiers_in_feature_postconditions(
        workspace: &Workspace,
        class_name: &ClassName,
        feature: &Feature,
    ) -> Self {
        let formatted_current_model_in_prestate =
            Self::format_model_of_class(workspace, class_name)
                .format_prestate()
                .format_inline_and_append();

        let formatted_current_model =
            Self::format_model_of_class(workspace, class_name).format_inline_and_append();

        let formatted_parameters_in_prestate = Self::format_parameters(feature.parameters())
            .format_prestate()
            .format_inline_and_append();

        let formatted_parameters =
            Self::format_parameters(feature.parameters()).format_inline_and_append();

        let formatted_return_type = feature
            .return_type()
            .map_or_else(String::new, |ty| format!(", Result: {ty}"));

        Self(format!(
            "Top level identifiers available in the pre-state of the postcondition: Current: {class_name}{formatted_current_model_in_prestate}{formatted_parameters_in_prestate}.\nIdentifiers available in the post-state for the postcondition: Current: {class_name}{formatted_current_model}{formatted_parameters}{formatted_return_type}."
        ))
    }

    fn format_available_identifiers_in_feature_preconditon(
        workspace: &Workspace,
        class_name: &ClassName,
        feature: &Feature,
    ) -> Self {
        let formatted_current_model =
            Self::format_model_of_class(workspace, class_name).format_inline_and_append();

        let formatted_parameters =
            Self::format_parameters(feature.parameters()).format_inline_and_append();

        Self(format!(
            "Top level identifiers available in the preconditions: Current: {class_name}{formatted_current_model}{formatted_parameters }.\n"
        ))
    }

    fn format_model_of_class(workspace: &Workspace, class_name: &ClassName) -> Self {
        match class_name.inhereted_model(workspace.system_classes()) {
            Some(model) if model.is_empty() => Self(String::new()),
            None => Self(String::new()),
            Some(model) => Self(format!("{model}")),
        }
    }

    fn format_inline_and_append(self) -> Self {
        match self {
            Self(ref text) if text.is_empty() => self,
            Self(text) => Source(format!(", {}", text.trim_end().replace("\n", ", "))),
        }
    }

    fn format_model_of_parameters(workspace: &Workspace, parameters: &FeatureParameters) -> Self {
        Self(parameters.formatted_model(workspace.system_classes()))
    }

    fn format_parameters(parameters: &FeatureParameters) -> Self {
        if parameters.is_empty() {
            Source(String::new())
        } else {
            Source(format!("{parameters}"))
        }
    }

    fn format_prestate(&self) -> Self {
        let Source(text) = self;

        Source(text.lines().fold(String::new(), |mut acc, line| {
            acc.push_str("old ");
            acc.push_str(line);
            acc.push('\n');
            acc
        }))
    }

    fn prepend_if_nonempty<S: AsRef<str>>(self, prefix: S) -> Self {
        match self {
            Self(ref text) if !text.is_empty() => Source(format!("{}{}", prefix.as_ref(), text)),
            _ => self,
        }
    }
}

struct Injection(Point, Source);

struct SystemMessage(String);

impl SystemMessage {
    fn default_for_model_based_contracts<T: ToString>() -> Self {
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

fn immediate_feature_injections(
    workspace: &Workspace,
    class_name: &ClassName,
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
    feature_available_identifiers_injections(workspace, class_name, feature)
        .into_iter()
        .chain(immediate_feature_contracts_injections(feature))
}

fn feature_available_identifiers_injections(
    workspace: &Workspace,
    class_name: &ClassName,
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
    let feature_start = feature.range().start;

    [
        Injection(
            feature_start,
            Source::format_model_of_class(workspace, class_name)
                .prepend_if_nonempty("Model of the current class both immediate and inherited: ")
                .comment(),
        ),
        Injection(
            feature_start,
            Source::format_model_of_parameters(workspace, feature.parameters()).comment(),
        ),
        Injection(
            feature_start,
            Source::format_available_identifiers_in_feature_preconditon(
                workspace, class_name, feature,
            )
            .comment(),
        ),
        Injection(
            feature_start,
            Source::format_available_identifiers_in_feature_postconditions(
                workspace, class_name, feature,
            )
            .comment(),
        ),
    ]
}

fn immediate_feature_contracts_injections(
    feature: &Feature,
) -> impl IntoIterator<Item = Injection> {
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

    feature
        .point_end_preconditions()
        .map(|point| Injection(point, Source(hole_preconditions)))
        .into_iter()
        .chain(
            feature
                .point_end_postconditions()
                .map(|point| Injection(point, Source(hole_postconditions))),
        )
}

fn class_injections(workspace: &Workspace, class: &Class) -> impl IntoIterator<Item = Injection> {
    class
        .features()
        .into_iter()
        .flat_map(|feature| immediate_feature_injections(workspace, class.name(), feature))
}

fn sort_injections(injections: &mut [Injection]) {
    injections.sort_by(
        |Injection(Point { row, column }, _),
         Injection(
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

fn inject_into_source(mut injections: Vec<Injection>, source: Source) -> Source {
    sort_injections(&mut injections);

    let Source(source_content) = source;
    let mut text = String::new();
    for (linenum, line) in source_content.lines().enumerate() {
        // Select injections of current line;
        // Relies on ordering of injections;
        let mut current_injections =
            injections
                .iter()
                .filter_map(|&Injection(Point { row, column }, ref text)| {
                    (row == linenum).then_some((column, text))
                });
        // If there are no injections, add line to the text.
        let Some((mut oc, Source(oi))) = current_injections.next() else {
            text.push_str(line);
            text.push('\n');
            continue;
        };

        text.push_str(&line[..oc]);
        text.push_str(oi);
        for (nc, Source(ni)) in current_injections {
            text.push_str(&line[oc..nc]);
            text.push_str(ni);
            oc = nc;
        }

        text.push_str(&line[oc..]);
        text.push('\n');
    }
    Source(text)
}

pub struct ClassPrompt(String);

impl ClassPrompt {
    async fn try_new(workspace: &Workspace, class: &Class) -> Option<Self> {
        let injections = class_injections(workspace, class);
        let source = source(workspace.path(class.name())).await?;

        let Source(prompt_text) = inject_into_source(injections.into_iter().collect(), source);
        Some(Self(prompt_text))
    }
}
