use crate::code_entities::prelude::*;
use crate::eiffel_source::Indent;
use anyhow::Context;
use anyhow::Result;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::path::Path;

mod model_based_contracts;
mod routine_fixes;

#[derive(Debug, Clone)]
pub struct Prompt {
    system_message: String,
    source: String,
    /// Pairs to be inserted in the source; the left is the point-offset; the right is the string to insert.
    injections: Vec<(Point, String)>,
}

impl Prompt {
    fn default_for_routine_fixes() -> Self {
        Self {
            system_message: (String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language in writing formally verified code.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive an eiffel snippet with a comment identifying the routine to fix which contains an error message of AutoProof.
Respond with a correct version of the same routine.
"#,
            )),
            source: String::new(),
            injections: Vec::new(),
        }
    }
}

impl Prompt {
    pub async fn feature_fixes(
        feature: &Feature,
        filepath: &Path,
        error_message: String,
    ) -> Result<Self> {
        let mut var = Self::default_for_routine_fixes();
        var.set_feature_src(feature, filepath).await?;
        var.add_commented_injection_after_feature(feature, error_message)
            .await;
        var.add_commented_injection_at_the_beginning("The following feature does not verify, there is a bug in the body in its body. Fix the body of following feature. After the code you will find a comment with the error message from AutoProof.");
        Ok(var)
    }

    fn set_source<S: Borrow<str>>(&mut self, source: S) {
        self.source.clear();
        self.source.push_str(source.borrow());
    }

    async fn set_feature_src(&mut self, feature: &Feature, filepath: &Path) -> Result<()> {
        let feature_src = feature.source_unchecked(filepath).await?;
        self.set_source(feature_src);
        Ok(())
    }

    fn add_commented_injection_at_the_beginning<T: ToString>(&mut self, message: T) {
        let normalized_begin = Point { row: 0, column: 0 };
        let fmt_message = Self::eiffel_comment(message.to_string());

        self.injections.push((normalized_begin, fmt_message));
    }

    async fn add_commented_injection_after_feature(&mut self, feature: &Feature, message: String) {
        let Range { start, end } = feature.range();

        self.injections
            .push((*end - *start, Self::eiffel_comment(message)));
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

    pub fn into_llm_chat_messages(self) -> Vec<super::constructor_api::MessageOut> {
        let text = Prompt::inject_into_source(self.injections, self.source);

        let val = vec![
            super::constructor_api::MessageOut::new_system(self.system_message),
            super::constructor_api::MessageOut::new_user(text),
        ];
        val
    }
}
