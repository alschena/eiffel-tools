use crate::lib::code_entities::prelude::*;
use crate::lib::eiffel_source::Indent;
use anyhow::Context;
use std::cmp::Ordering;
use std::path::Path;
use tracing::info;
use tracing::warn;

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
        system_classes: &[Class],
    ) -> anyhow::Result<Self> {
        let mut var = Self::default_for_routine_fixes();
        var.set_feature_src(feature, filepath).await?;
        var.add_autoproof_error_message_injection(feature, todo!())
            .await?;
        todo!()
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

    async fn add_autoproof_error_message_injection(
        &mut self,
        feature: &Feature,
        class_name: &ClassName,
    ) -> anyhow::Result<()> {
        let end_point = feature.range().end;

        let autoproof = std::process::Command::new("ec")
            .arg("-autoproof")
            .arg("CLASS.feature")
            .output()
            .with_context(|| "fails to run the autoproof command: `ec -autoproof CLASS.feature`")?;

        let stderr_autoproof = autoproof.stderr;
        let stdout_autoproof = autoproof.stdout;

        if !stderr_autoproof.is_empty() {
            warn!(
                "AutProof counterexample goes into stderr: {:#?}",
                stderr_autoproof
            );
        }

        if !stdout_autoproof.is_empty() {
            warn!(
                "AutProof counterexample goes into stdout: {:#?}",
                stdout_autoproof
            );
        }

        let prefix = "\nThis is the counterexample AutoProof provides: "
            .as_bytes()
            .into_iter()
            .copied();

        let message: Vec<_> = prefix
            .chain(stderr_autoproof.into_iter())
            .chain(stdout_autoproof.into_iter())
            .collect();

        let fmt_message = Self::eiffel_comment(String::from_utf8(message)?);

        self.injections.push((end_point, fmt_message));
        Ok(())
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
