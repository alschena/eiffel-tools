use super::Command;
use crate::lib::code_entities::prelude::Class;
use crate::lib::code_entities::prelude::Feature;
use crate::lib::code_entities::prelude::Range;
use crate::lib::eiffel_source::Indent;
use crate::lib::language_server_protocol::commands::lsp_types;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

mod daikon_types;
use daikon_types::DaikonDecType;
use daikon_types::DaikonPosition;
use daikon_types::DaikonRepType;
use daikon_types::DaikonVarKind;

#[derive(Debug, Clone)]
pub struct DaikonInstrumenter<'ws> {
    workspace: &'ws Workspace,
    filepath: &'ws Path,
    class: &'ws Class,
    feature: &'ws Feature,
}

impl<'ws> DaikonInstrumenter<'ws> {
    pub fn try_new(
        workspace: &'ws Workspace,
        filepath: &'ws Path,
        feature_name: &str,
    ) -> Result<Self> {
        let file = workspace
            .find_file(filepath)
            .with_context(|| format!("Fails to find file of path: {filepath:#?}"))?;
        let class = file.class();
        let feature = class
            .features()
            .iter()
            .find(|&ft| ft.name() == feature_name)
            .with_context(|| {
                format!("Fails to find in file: {file:#?} feature of name: {feature_name}")
            })?;
        Ok(Self {
            class,
            workspace,
            filepath,
            feature,
        })
    }

    async fn write_declaration(&self) -> Result<()> {
        let mut declaration_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.declaration_file()?)
            .await?;

        let version = format!("decl-version 2.0\n");
        let declarations = self.feature_declaration()?;

        declaration_file
            .write(format!("{version}\n{declarations}").as_bytes())
            .await?;
        declaration_file.flush().await?;

        Ok(())
    }

    fn declaration_file(&self) -> Result<PathBuf> {
        let mut pathbuf = self.filepath.to_owned().to_path_buf();
        pathbuf.set_extension("decls");
        Ok(pathbuf)
    }

    fn feature_parameters_declarations(&self) -> Result<String> {
        let feature = self.feature;
        let feature_parameters = feature.parameters();
        let feature_parameters_types = feature.parameters().types();
        let parameters_names = feature_parameters.names();
        let parameters_daikon_var_kinds: Vec<DaikonVarKind> = feature_parameters.try_into()?;
        let parameters_daikon_dec_types = feature_parameters_types
            .iter()
            .map(|ty| DaikonDecType::try_from(ty));
        let parameters_daikon_rep_types = feature_parameters_types
            .iter()
            .map(|ty| DaikonRepType::try_from(ty));

        parameters_names
            .iter()
            .zip(parameters_daikon_var_kinds)
            .zip(parameters_daikon_dec_types)
            .zip(parameters_daikon_rep_types)
            .fold(
                Ok(String::new()),
                |acc: Result<String>, (((name, var_kind), dec_type), rep_type)| {
                    let acc = acc?;
                    let dec_type = dec_type?;
                    let rep_type = rep_type?;
                    Ok(format!(
                        r#"{acc}
variable {name}
{var_kind}
{dec_type}
{rep_type}"#
                    ))
                },
            )
    }

    fn feature_return_declaration(&self) -> Result<String> {
        match self.feature.return_type() {
            Some(ret_type) => {
                let dec_type: DaikonDecType = ret_type.try_into()?;
                let rep_type: DaikonRepType = ret_type.try_into()?;
                let var_kind = DaikonVarKind::Return;
                Ok(format!(
                    r#"
variable return
{var_kind}
{dec_type}
{rep_type}"#
                ))
            }
            None => Ok(String::new()),
        }
    }

    // TODO: use `var-kind field` with the required `enclosing-var Current` (if it works with unqualified calls),i.e. implement constructor for fields in `DaikonVarKind`.
    fn class_fields_declaration(&self) -> Result<String> {
        let system_classes = self.workspace.system_classes();
        let class_fields: Vec<_> = self
            .class
            .immediate_and_inherited_features(&system_classes)
            .into_iter()
            .filter_map(|ft| {
                (ft.parameters().is_empty() && ft.return_type().is_some()).then_some(ft)
            })
            .collect();

        class_fields.into_iter().fold(Ok(String::new()), |acc, ft| {
            let acc = acc?;
            let Some(ft_type) = ft.return_type() else {
                unreachable!("fails to get type of attribute")
            };
            let name = ft.name();
            let dec_type: DaikonDecType = ft_type.try_into()?;
            let rep_type: DaikonRepType = ft_type.try_into()?;
            Ok(format!(
                r#"{acc}
variable {name}
	var-kind variable
{dec_type} 
{rep_type}"#
            ))
        })
    }

    // TODO: Add `Current` to declared variables.
    fn feature_declaration(&self) -> Result<String> {
        let class_name = self.class.name();
        let feature = self.feature;
        let feature_name = feature.name();
        let class_fields_declaration = self.class_fields_declaration()?;
        let parameters_declaration = self.feature_parameters_declarations()?;
        let return_declaration = self.feature_return_declaration()?;

        let full_declaration = format!(
            r#"ppt {class_name}.{feature_name}::ENTER
ppt-type enter{class_fields_declaration}{parameters_declaration}

ppt {class_name}.{feature_name}::EXIT
ppt-type exit{class_fields_declaration}{parameters_declaration}{return_declaration}"#
        );
        Ok(full_declaration)
    }

    fn class_fields_instrumentation(&self) -> String {
        let system_classes = self.workspace.system_classes();
        let class_fields: Vec<_> = self
            .class
            .immediate_and_inherited_features(&system_classes)
            .into_iter()
            .filter_map(|ft| {
                (ft.parameters().is_empty() && ft.return_type().is_some()).then_some(ft)
            })
            .collect();
        let indentation_string = Self::eiffel_statement_indentation_string();

        class_fields.iter().fold(String::new(), |acc, ft| {
            format!(
                r#"{acc}
{indentation_string}io.put_string("{}")
{indentation_string}io.new_line
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line
{indentation_string}io.put_string("1")
{indentation_string}io.new_line"#,
                ft.name(),
                ft.name()
            )
        })
    }

    fn feature_parameter_instrumentation(&self) -> String {
        let indentation_string = Self::eiffel_statement_indentation_string();

        self.feature
            .parameters()
            .names()
            .iter()
            .fold(String::new(), |acc, param_name| {
                format!(
                    r#"{acc}
{indentation_string}io.put_string("{}")
{indentation_string}io.new_line
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line
{indentation_string}io.put_string("1")
{indentation_string}io.new_line"#,
                    param_name, param_name
                )
            })
    }

    fn eiffel_statement_indentation_string() -> String {
        "\t\t".to_string()
    }

    fn feature_instrumentation_at(&self, pos: DaikonPosition) -> String {
        let class_name = self.class.name();
        let feature_name = self.feature.name();
        let indentation_string = Self::eiffel_statement_indentation_string();
        let class_fields = self.class_fields_instrumentation();
        let parameters = self.feature_parameter_instrumentation();
        format!(
            r#"
{indentation_string}io.put_string("{class_name}.{feature_name}::{pos}")
{indentation_string}io.new_line
{class_fields}
{parameters}
            "#
        )
    }

    pub fn instrument_body_start_and_end(&self) -> Result<[lsp_types::TextEdit; 2]> {
        let Some(Range { mut start, end }) = self.feature.body_range().cloned() else {
            bail!(
                "fails find the range of the body of the feature to instrument: {:#?}",
                &self.feature
            )
        };
        start.shift_right(2); // Move start point after word `do`

        Ok([
            lsp_types::TextEdit {
                range: Range::new_collapsed(start).try_into()?,
                new_text: self.feature_instrumentation_at(DaikonPosition::Enter),
            },
            lsp_types::TextEdit {
                range: Range::new_collapsed(end).try_into()?,
                new_text: self.feature_instrumentation_at(DaikonPosition::Exit),
            },
        ])
    }

    fn instrumented_redefinition(&self) -> String {
        let indentation_string = Self::eiffel_statement_indentation_string();
        let instrumentation_body_start = self.feature_instrumentation_at(DaikonPosition::Enter);
        let instrumentation_body_end = self.feature_instrumentation_at(DaikonPosition::Exit);
        let feature_name = self.feature.name();
        let return_type_text = self
            .feature
            .return_type()
            .map(|ty| format!(": {ty}"))
            .unwrap_or_default();
        let feature_parameters = self.feature.parameters();
        let comma_separated_parameters =
            feature_parameters
                .names()
                .iter()
                .fold(String::new(), |acc, param_name| {
                    if acc.is_empty() {
                        format!("{param_name}")
                    } else {
                        format!("{acc}, {param_name}")
                    }
                });
        let wrapped_comma_separated_parameters =
            format!("({})", feature_parameters.to_string().trim());
        format!(
            r#"
    {feature_name} {wrapped_comma_separated_parameters}{return_type_text}
        do
{instrumentation_body_start}
{indentation_string}Precursor ({comma_separated_parameters})
{instrumentation_body_end}
        end

            "#
        )
    }

    pub fn instrumentation_by_subclass(&self) -> String {
        let class_name = self.class.name();
        let feature_name = self.feature.name();
        let instrumented_redefinition = self.instrumented_redefinition();
        format!(
            r#"class
    {class_name}_DAIKON_INSTRUMENTED
inherit
    {class_name}
    redefine
        {feature_name}
    end
feature
{instrumented_redefinition}
end
"#
        )
    }
}

impl<'ws> TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> for DaikonInstrumenter<'ws> {
    type Error = anyhow::Error;

    fn try_from(value: (&'ws Workspace, Vec<serde_json::Value>)) -> Result<Self, Self::Error> {
        let workspace = value.0;
        let mut arguments = value.1;
        let feature_name = arguments.pop().with_context(|| {
            "Fails to retrieve the second argument (feature name) to add routine specification."
        })?;
        let feature_name: String = serde_json::from_value(feature_name)?;
        let filepath = arguments.pop().with_context(|| {
            "Fails to retrieve the first argument (file path) to add routine specification."
        })?;
        let filepath: PathBuf = serde_json::from_value(filepath)?;
        let filepath_validated = workspace
            .find_file(&filepath)
            .map(|file| file.path())
            .with_context(|| {
                format!("fails to find file at path: {:#?} in workspace. ", filepath)
            })?;
        Self::try_new(workspace, &filepath_validated, &feature_name)
    }
}

impl<'ws> Command<'ws> for DaikonInstrumenter<'ws> {
    const NAME: &'static str = "instrument_feature_for_daikon";

    const TITLE: &'static str = "Instrument feature for Daikon";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let Ok(serialized_filepath) = serde_json::to_value(self.filepath) else {
            unreachable!("fails to serialize path: {:#?}", self.filepath)
        };
        let feature = self.feature;
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn side_effect(&self) -> anyhow::Result<()> {
        self.write_declaration().await
    }

    async fn generate_edits(
        &self,
        _generators: &crate::lib::generators::Generators,
    ) -> Result<lsp_types::WorkspaceEdit> {
        let url = lsp_types::Url::from_file_path(self.filepath).map_err(|_| {
            anyhow!("if on unix path must be absolute. if on windows path must have disk prefix")
        })?;

        Ok(lsp_types::WorkspaceEdit::new(HashMap::from([(
            url,
            self.instrument_body_start_and_end()?.into(),
        )])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::code_entities::prelude::*;
    use crate::lib::parser::Parser;
    use crate::lib::processed_file::ProcessedFile;
    use crate::lib::workspace::Workspace;
    use anyhow::bail;
    use anyhow::Context;
    use anyhow::Result;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    use super::DaikonInstrumenter;

    async fn processed_file() -> ProcessedFile {
        let mut parser = Parser::new();
        let temp_dir = TempDir::new().expect("must create temporary directory.");
        let file = temp_dir.child("processed_file_new.e");
        file.write_str(
            r#"class
    TEST
feature
    sum (x,y: INTEGER): INTEGER
        do
            Result := x + y
        end
end
"#,
        )
        .expect("write to file");
        parser
            .processed_file(file.to_path_buf())
            .await
            .expect("fails to create processed file")
    }

    impl<'ws> DaikonInstrumenter<'ws> {
        pub async fn mock(mock_workspace: &'ws mut Workspace) -> Result<Self> {
            let processed_file = processed_file().await;
            let filepath = processed_file.path();
            mock_workspace.set_files(vec![processed_file.clone()]);
            let file = mock_workspace.find_file(filepath).with_context(|| {
                format!("fails to find file of path {:#?} in workspace.", filepath)
            })?;
            let filepath = file.path();
            let class = file.class();
            let feature = class
                .features()
                .first()
                .with_context(|| format!("fails to find feature in class: {:#?}", class))?;

            Ok(DaikonInstrumenter {
                workspace: mock_workspace,
                filepath,
                class,
                feature,
            })
        }
    }

    #[tokio::test]
    async fn declaration_file() -> Result<()> {
        let ws = &mut Workspace::mock();
        let daikon_instrumenter = DaikonInstrumenter::mock(ws).await?;

        assert_eq!(
            daikon_instrumenter.declaration_file()?.parent(),
            daikon_instrumenter.filepath.parent()
        );
        assert_eq!(
            daikon_instrumenter.declaration_file()?.file_stem(),
            daikon_instrumenter.filepath.file_stem()
        );
        assert!(daikon_instrumenter
            .declaration_file()?
            .extension()
            .is_some_and(|ext| ext == "decls"));
        Ok(())
    }

    #[tokio::test]
    async fn instrument_body_start_and_end() -> Result<()> {
        let workspace = &mut Workspace::mock();
        let processed_file = processed_file().await;
        let filepath = processed_file.path();
        let class = &processed_file.class().clone();
        let Some(ref feature) = class.features().first() else {
            bail!("fails to find feature")
        };
        workspace.set_files(vec![processed_file.clone()]);

        let daikon_instrumenter = DaikonInstrumenter {
            workspace,
            filepath,
            class,
            feature,
        };

        let [start_edit, end_edit] = daikon_instrumenter.instrument_body_start_and_end()?;

        let start_edit_iter = start_edit
            .new_text
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());
        let end_edit_iter = end_edit
            .new_text
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());

        let oracle_at_start = r#"
			io.put_string("TEST.sum::ENTER")
			io.new_line
			io.put_string("x")
			io.new_line
			io.put_string(x.out)
			io.new_line
			io.put_string("1")
			io.new_line
			io.put_string("y")
			io.new_line
			io.put_string(y.out)
			io.new_line
			io.put_string("1")
			io.new_line"#;
        let oracle_at_end = r#"
			io.put_string("TEST.sum::EXIT")
			io.new_line
			io.put_string("x")
			io.new_line
			io.put_string(x.out)
			io.new_line
			io.put_string("1")
			io.new_line
			io.put_string("y")
			io.new_line
			io.put_string(y.out)
			io.new_line
			io.put_string("1")
			io.new_line"#;
        let oracle_at_start_iter = oracle_at_start
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());
        let oracle_at_end_iter = oracle_at_end
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());

        let same_start = oracle_at_start_iter
            .zip(start_edit_iter)
            .all(|(or, ac)| or == ac);
        let same_end = oracle_at_end_iter
            .zip(end_edit_iter)
            .all(|(or, ac)| or == ac);

        assert!(
            same_start,
            "oracle: {oracle_at_start}\nresult: {}",
            start_edit.new_text
        );
        assert!(
            same_end,
            "oracle: {oracle_at_end}\nresult: {}",
            end_edit.new_text
        );
        assert_eq!(
            start_edit.range,
            Range::new_collapsed(Point { row: 4, column: 10 }).try_into()?
        );
        assert_eq!(
            end_edit.range,
            Range::new_collapsed(Point { row: 5, column: 27 }).try_into()?
        );
        Ok(())
    }

    #[tokio::test]
    async fn daikon_declarations() -> Result<()> {
        let mut ws = Workspace::mock();
        let daikon_instrumenter = DaikonInstrumenter::mock(&mut ws).await?;

        let declarations = daikon_instrumenter.feature_declaration()?;
        eprintln!("{declarations}");
        assert_eq!(
            declarations.trim(),
            r#"ppt TEST.sum::ENTER
ppt-type enter
variable x
	var-kind variable
	dec-type int
	rep-type int
variable y
	var-kind variable
	dec-type int
	rep-type int

ppt TEST.sum::EXIT
ppt-type exit
variable x
	var-kind variable
	dec-type int
	rep-type int
variable y
	var-kind variable
	dec-type int
	rep-type int
variable return
	var-kind return
	dec-type int
	rep-type int"#
        );

        Ok(())
    }

    #[tokio::test]
    async fn instrumented_redefinition() -> Result<()> {
        let mut ws = Workspace::mock();
        let dkn = DaikonInstrumenter::mock(&mut ws).await?;
        let res = dkn.instrumented_redefinition();
        let instrumentation_body_start = dkn.feature_instrumentation_at(DaikonPosition::Enter);
        let instrumentation_body_end = dkn.feature_instrumentation_at(DaikonPosition::Exit);

        let oracle = format!(
            r#"
sum (x: INTEGER
    y: INTEGER): INTEGER
    do
    {instrumentation_body_start}
        Precursor (x, y)
    {instrumentation_body_end}
    end"#
        );
        let oracle_iter = oracle
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());

        let res_iter = res.lines().map(|ln| ln.trim()).filter(|ln| !ln.is_empty());

        let same = oracle_iter.zip(res_iter).all(|(or, rs)| or == rs);
        assert!(same, "oracle: {oracle}\nres: {res}");

        Ok(())
    }

    #[tokio::test]
    async fn instrumented_class_text() -> Result<()> {
        let mut ws = Workspace::mock();
        let dkn = DaikonInstrumenter::mock(&mut ws).await?;

        let res = dkn.instrumentation_by_subclass();
        let instrumented_redefinition = dkn.instrumented_redefinition();

        let oracle_res = format!(
            r#"
class
	TEST_DAIKON_INSTRUMENTED
inherit
	TEST
	redefine
		sum
	end
feature
{instrumented_redefinition}
end
"#
        );

        let oracle_res_clean = oracle_res
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());
        let res_clean = res.lines().map(|ln| ln.trim()).filter(|ln| !ln.is_empty());

        let same = oracle_res_clean.zip(res_clean).all(|(or, ac)| or == ac);
        assert!(same, "oracle_res: {oracle_res}\nres: {res}");

        Ok(())
    }
}
