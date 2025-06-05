use crate::code_entities::prelude::*;
use crate::generators::constructor_api;
use crate::workspace::Workspace;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;

mod feature_prompt;
pub use feature_prompt::FeaturePrompt;

mod class_prompt;
pub use class_prompt::ClassPrompt;

#[derive(Debug, Clone, PartialEq, Eq)]
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

    fn indent(self) -> Self {
        let Source(text) = self;
        Source(text.lines().fold(String::new(), |mut acc, line| {
            if !line.trim_start().is_empty() {
                acc.push_str("\t");
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
        )).comment()
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
        )).comment()
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
        Self(parameters.formatted_model(workspace.system_classes())).comment()
    }

    fn format_parameters(parameters: &FeatureParameters) -> Self {
        if parameters.is_empty() {
            Source(String::new())
        } else {
            Source(format!("{parameters}")).comment()
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

#[derive(Debug, Clone)]
struct Injection(Point, Source);

#[derive(Debug, Clone)]
struct SystemMessage(String);

impl From<SystemMessage> for constructor_api::MessageOut {
    fn from(value: SystemMessage) -> Self {
        let SystemMessage(content) = value;
        constructor_api::MessageOut::new_user(content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserMessage(String);

impl From<Source> for UserMessage {
    fn from(value: Source) -> Self {
        let Source(content) = value;
        Self(content)
    }
}

impl From<UserMessage> for constructor_api::MessageOut {
    fn from(value: UserMessage) -> Self {
        let UserMessage(content) = value;
        constructor_api::MessageOut::new_user(content)
    }
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

fn injected_into_source(mut injections: Vec<Injection>, source: Source) -> Source {
    sort_injections(&mut injections);

    let Source(source_content) = source;
    let mut text = String::new();
    for (linenum, line) in source_content.lines().enumerate() {
        eprintln!(
            "linenum: {} length: {} content: {}",
            linenum,
            line.len(),
            line
        );
        // Select injections of current line;
        // Relies on ordering of injections;
        let mut current_injections = injections
            .iter()
            .filter_map(|&Injection(Point { row, column }, ref text)| {
                (row == linenum).then_some((column, text))
            })
            .inspect(|inj| {
                eprintln!(
                    "Injection: linenum: {} length: {} injection: {:#?}",
                    linenum,
                    line.len(),
                    inj
                )
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
