use super::Command;
use crate::lib::code_entities::prelude::Class;
use crate::lib::code_entities::prelude::EiffelType;
use crate::lib::code_entities::prelude::Feature;
use crate::lib::code_entities::prelude::FeatureParameters;
use crate::lib::code_entities::prelude::Range;
use crate::lib::code_entities::Indent;
use crate::lib::language_server_protocol::commands::lsp_types;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::workspace::Workspace;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaikonInstrumenter<'ws> {
    workspace: &'ws Workspace,
    file: &'ws ProcessedFile,
    class: &'ws Class,
    feature: &'ws Feature,
}

enum DaikonVarKind {
    Field,
    Function,
    Array,
    Variable,
    Return,
}

impl DaikonVarKind {
    fn from_feature_return_type(ft: &Feature) -> Self {
        assert!(ft.return_type().is_some());
        DaikonVarKind::Return
    }
}

impl Display for DaikonVarKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DaikonVarKind::Field => "field",
            DaikonVarKind::Function => "function",
            DaikonVarKind::Array => "array",
            DaikonVarKind::Variable => "variable",
            DaikonVarKind::Return => "return",
        };
        write!(f, "\tvar-kind {}", text)
    }
}

impl TryFrom<&FeatureParameters> for Vec<DaikonVarKind> {
    type Error = anyhow::Error;
    fn try_from(value: &FeatureParameters) -> Result<Self> {
        value
            .types()
            .iter()
            .map(|ty| {
                ty.class_name().map(|class_name| {
                    if class_name.to_string().to_lowercase().contains("array") {
                        DaikonVarKind::Array
                    } else {
                        DaikonVarKind::Variable
                    }
                })
            })
            .collect::<Result<Vec<_>>>()
    }
}

enum DaikonDecType {
    Int,
    Boolean,
    String,
    Custom(String),
}

impl TryFrom<&EiffelType> for DaikonDecType {
    type Error = anyhow::Error;

    fn try_from(value: &EiffelType) -> std::result::Result<Self, Self::Error> {
        value
            .class_name()
            .map(|class_name| match class_name.0.as_str() {
                "BOOLEAN" => DaikonDecType::Boolean,
                "INTEGER" => DaikonDecType::Int,
                "STRING" => DaikonDecType::String,
                otherwise @ _ => DaikonDecType::Custom(otherwise.to_string()),
            })
    }
}

impl Display for DaikonDecType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DaikonDecType::Int => "int",
            DaikonDecType::Boolean => "boolean",
            DaikonDecType::String => "java.lang.String",
            DaikonDecType::Custom(s) => &s,
        };
        write!(f, "\tdec-type {}", text)
    }
}

enum DaikonRepType {
    Boolean,
    Int,
    HashCode,
    Double,
    String,
    Array(Box<DaikonRepType>),
}

impl TryFrom<&EiffelType> for DaikonRepType {
    type Error = anyhow::Error;

    fn try_from(value: &EiffelType) -> std::result::Result<Self, Self::Error> {
        value
            .class_name()
            .map(|class_name| match class_name.0.as_str() {
                "BOOLEAN" => DaikonRepType::Boolean,
                "INTEGER" => DaikonRepType::Int,
                "REAL" => DaikonRepType::Double,
                "STRING" => DaikonRepType::String,
                custom @ _ if custom.to_lowercase().contains("array") => {
                    DaikonRepType::Array(Box::new(DaikonRepType::HashCode))
                }
                _ => DaikonRepType::HashCode,
            })
    }
}

impl Display for DaikonRepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DaikonRepType::Boolean => "boolean".to_string(),
            DaikonRepType::Int => "int".to_string(),
            DaikonRepType::HashCode => "hashcode".to_string(),
            DaikonRepType::Double => "double".to_string(),
            DaikonRepType::String => "java.lang.String".to_string(),
            DaikonRepType::Array(base_type) if !matches!(**base_type, Self::Array(_)) => {
                format!("{base_type}")
            }
            _ => unreachable!(),
        };

        write!(f, "\trep-type {text}")
    }
}

impl<'ws> DaikonInstrumenter<'ws> {
    pub fn try_new(workspace: &'ws Workspace, filepath: &Path, feature_name: &str) -> Result<Self> {
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
            file,
            feature,
        })
    }

    fn declaration_file(&self) -> Result<PathBuf> {
        let mut pathbuf = self.file.path().to_path_buf();
        pathbuf.set_extension("decls");
        Ok(pathbuf)
    }

    fn feature_declaration(&self) -> Result<String> {
        let class_name = self.class.name();
        let feature = self.feature;
        let feature_name = feature.name();
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
        let parameters_declaration = parameters_names
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
{rep_type}
"#
                    ))
                },
            )?;
        let return_declaration = match feature.return_type() {
            Some(ret_type) => {
                let dec_type: DaikonDecType = ret_type.try_into()?;
                let rep_type: DaikonRepType = ret_type.try_into()?;
                let var_kind = DaikonVarKind::from_feature_return_type(feature);
                format!(
                    r#"
variable return
{var_kind}
{dec_type}
{rep_type}
"#
                )
            }
            None => String::new(),
        };

        let full_declaration = format!(
            r#"DECLARE
ppt {class_name}.{feature_name}::ENTER
ppt-type enter
{parameters_declaration}

ppt {class_name}.{feature_name}::EXIT
ppt-type exit
{parameters_declaration}{return_declaration}
"#
        );
        Ok(full_declaration)
    }

    pub fn instrument_body_start_and_end(&self) -> Result<[lsp_types::TextEdit; 2]> {
        let system_classes = self.workspace.system_classes();
        let class_fields: Vec<_> = self
            .class
            .immediate_and_inherited_features(&system_classes)
            .into_iter()
            .filter_map(|ft| {
                (ft.parameters().is_empty() && ft.return_type().is_some()).then_some(ft)
            })
            .collect();
        let parameters = self.feature.parameters();

        let Some(Range { mut start, end }) = self.feature.body_range().cloned() else {
            bail!(
                "fails find the range of the body of the feature to instrument: {:#?}",
                &self.feature
            )
        };

        let indentation_string =
            (0..=Feature::INDENTATION_LEVEL + 1).fold(String::new(), |acc, _| format!("{acc}\t"));

        let print_class_fields_instructions = class_fields.iter().fold(String::new(), |acc, ft| {
            format!(
                r#"{acc}
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line"#,
                ft.name()
            )
        });

        let print_class_fields_and_parameters_instructions =
            parameters
                .names()
                .iter()
                .fold(print_class_fields_instructions, |acc, param_name| {
                    format!(
                        r#"{acc}
{indentation_string}io.put_string({}.out)
{indentation_string}io.new_line"#,
                        param_name
                    )
                });

        start.shift_right(2); // Move start point after word `do`
        let collapsed_start_range = Range::new_collapsed(start);
        let text_edit_start = lsp_types::TextEdit {
            range: collapsed_start_range.try_into()?,
            new_text: print_class_fields_and_parameters_instructions.clone(),
        };

        let collapsed_end_range = Range::new_collapsed(end);
        let text_edit_end = lsp_types::TextEdit {
            range: collapsed_end_range.try_into()?,
            new_text: print_class_fields_and_parameters_instructions,
        };

        Ok([text_edit_start, text_edit_end])
    }

    pub fn add_declarations(&self) -> Result<()> {
        Ok(())
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
        Self::try_new(workspace, &filepath, &feature_name)
    }
}

impl<'ws> Command<'ws> for DaikonInstrumenter<'ws> {
    const NAME: &'static str = "instrument_feature_for_daikon";

    const TITLE: &'static str = "Instrument feature for Daikon";

    fn arguments(&self) -> Vec<serde_json::Value> {
        let path = self.file.path();
        let Ok(serialized_filepath) = serde_json::to_value(path) else {
            unreachable!("fails to serialize path: {path:#?}")
        };
        let feature = self.feature;
        let Ok(serialized_feature_name) = serde_json::to_value(feature.name()) else {
            unreachable!("fails to serialize name of feature: {feature:#?}")
        };
        vec![serialized_filepath, serialized_feature_name]
    }

    async fn generate_edits(
        &self,
        _generators: &crate::lib::generators::Generators,
    ) -> Result<lsp_types::WorkspaceEdit> {
        let url = lsp_types::Url::from_file_path(self.file.path()).map_err(|_| {
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
    use crate::lib::code_entities::prelude::*;
    use crate::lib::language_server_protocol::commands::lsp_types;
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

    #[tokio::test]
    async fn instrument_body_start_and_end() -> Result<()> {
        let mut workspace = Workspace::mock();
        let processed_file = processed_file().await;
        let class = processed_file.class().clone();
        let Some(feature) = class.features().first() else {
            bail!("fails to find feature")
        };
        workspace.set_files(vec![processed_file.clone()]);

        let daikon_instrumenter = DaikonInstrumenter {
            workspace: &workspace,
            file: &processed_file,
            class: &class,
            feature: &feature,
        };

        let [start_edit, end_edit] = daikon_instrumenter.instrument_body_start_and_end()?;
        assert_eq!(
            start_edit,
            lsp_types::TextEdit {
                range: Range::new_collapsed(Point { row: 4, column: 10 }).try_into()?,
                new_text: format!("\n\t\t\tio.put_string(x.out)\n\t\t\tio.new_line\n\t\t\tio.put_string(y.out)\n\t\t\tio.new_line")
            }
        );
        assert_eq!(
            end_edit,
            lsp_types::TextEdit {
                range: Range::new_collapsed(Point { row: 5, column: 27 }).try_into()?,
                new_text: format!("\n\t\t\tio.put_string(x.out)\n\t\t\tio.new_line\n\t\t\tio.put_string(y.out)\n\t\t\tio.new_line")
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn daikon_declarations() -> Result<()> {
        let mut workspace = Workspace::mock();
        let processed_file = processed_file().await;
        let class = processed_file.class().clone();
        let Some(feature) = class.features().first() else {
            bail!("fails to find feature")
        };
        workspace.set_files(vec![processed_file.clone()]);

        let daikon_instrumenter = DaikonInstrumenter {
            workspace: &workspace,
            file: &processed_file,
            class: &class,
            feature: &feature,
        };

        let declarations = daikon_instrumenter.feature_declaration()?;
        eprintln!("{declarations}");
        assert_eq!(
            declarations.trim(),
            r#"DECLARE
ppt TEST.sum::ENTER
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
}
