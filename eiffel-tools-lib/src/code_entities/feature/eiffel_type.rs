use crate::code_entities::class::model::ModelExtended;
use crate::code_entities::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EiffelType {
    /// The first string is the whole string.
    /// The second string is the class name.
    ClassType(String, String),
    TupleType(String),
    Anchored(String),
}
impl Display for EiffelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            EiffelType::ClassType(s, _) => s,
            EiffelType::TupleType(s) => s,
            EiffelType::Anchored(s) => s,
        };
        write!(f, "{text}")
    }
}
impl EiffelType {
    pub fn class_name(&self) -> Result<ClassName> {
        match self {
            EiffelType::ClassType(_, s) => Ok(ClassName(s.to_owned())),
            EiffelType::TupleType(_) => bail!("tuple type"),
            EiffelType::Anchored(_) => bail!("anchored type"),
        }
    }
    pub fn class<'a, 'b: 'a>(
        &'a self,
        mut system_classes: impl Iterator<Item = &'b Class>,
    ) -> &'b Class {
        let class = system_classes
            .find(|&c| {
                self.class_name()
                    .is_ok_and(|ref class_name| class_name == c.name())
            })
            .unwrap_or_else(|| {
                panic!(
                    "parameters' class name: {:?} must be in system.",
                    self.class_name()
                )
            });
        class
    }
    pub fn is_terminal_for_model(&self) -> bool {
        self.class_name()
            .is_ok_and(|class_name| class_name.is_terminal_for_model())
    }
    pub fn model_extension(&self, system_classes: &[Class]) -> ModelExtended {
        if self.is_terminal_for_model() {
            return ModelExtended::Terminal;
        }
        let Ok(class_name): Result<ClassName, _> = self.to_owned().try_into() else {
            unimplemented!("eiffel type's model extension implemented only for class types.")
        };
        class_name.model_extended(system_classes)
    }
}

impl TryFrom<EiffelType> for ClassName {
    type Error = anyhow::Error;

    fn try_from(value: EiffelType) -> Result<Self, Self::Error> {
        value.class_name().map_err(|e| anyhow!("{e}"))
    }
}

impl From<ClassName> for EiffelType {
    fn from(value: ClassName) -> Self {
        let ClassName(name) = value;
        EiffelType::ClassType(name.clone(), name)
    }
}
