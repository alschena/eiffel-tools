use crate::code_entities::prelude::EiffelType;
use crate::code_entities::prelude::FeatureParameters;
use anyhow::Result;
use std::fmt::Display;

pub enum DaikonVarKind {
    Field,
    Function,
    Array,
    Variable,
    Return,
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
pub enum DaikonDecType {
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
            DaikonDecType::Custom(s) => s,
        };
        write!(f, "\tdec-type {}", text)
    }
}

pub enum DaikonRepType {
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

#[derive(Debug, Clone)]
pub enum DaikonPosition {
    Enter,
    Exit,
}

impl Display for DaikonPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DaikonPosition::Enter => "ENTER",
            DaikonPosition::Exit => "EXIT",
        };
        write!(f, "{text}")
    }
}
