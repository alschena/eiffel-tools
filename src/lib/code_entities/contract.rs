use super::prelude::*;
use std::fmt::Debug;
use std::fmt::Display;
use tracing::info;

mod blocks;
pub use blocks::Block;
pub use blocks::Postcondition;
pub use blocks::Precondition;
pub use blocks::RoutineSpecification;

mod clause;

#[derive(Debug)]
pub enum ValidityError {
    Syntax,
    Identifiers,
    Calls,
    Repetition,
}
pub(crate) trait Valid: Debug {
    fn validity(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        if !self.valid_syntax() {
            info!("invalid syntax: {self:#?}");
            return Err(ValidityError::Syntax);
        }
        if !self.valid_identifiers(system_classes, current_class, current_feature) {
            info!("invalid identifiers: {self:#?}");
            return Err(ValidityError::Identifiers);
        }
        if !self.valid_calls(system_classes, current_class) {
            info!("invalid calls: {self:#?}");
            return Err(ValidityError::Calls);
        }
        if !self.valid_no_repetition(system_classes, current_class, current_feature) {
            info!("invalid for repetition: {self:#?}");
            return Err(ValidityError::Repetition);
        }
        Ok(())
    }
    fn valid_syntax(&self) -> bool;
    fn valid_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool;
    fn valid_calls(&self, _system_classes: &[&Class], _current_class: &Class) -> bool {
        true
    }
    fn valid_no_repetition(
        &self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
}
pub(crate) trait Fix: Valid {
    fn fix(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        while let Err(e) = self.validity(system_classes, current_class, current_feature) {
            eprintln!("{e:#?}");
            match e {
                ValidityError::Syntax => {
                    self.fix_syntax(system_classes, current_class, current_feature)?;
                    info!("applied syntax fix to {self:#?}");
                }
                ValidityError::Identifiers => {
                    self.fix_identifiers(system_classes, current_class, current_feature)?;
                    info!("applied identifiers fix to {self:#?}");
                }
                ValidityError::Calls => {
                    self.fix_calls(system_classes, current_class, current_feature)?;
                    info!("applied calls fix to {self:#?}");
                }
                ValidityError::Repetition => {
                    self.fix_repetition(system_classes, current_class, current_feature)?;
                    info!("applied repetition fix to {self:#?}");
                }
            }
        }
        Ok(())
    }
    fn fix_syntax(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        return Err(ValidityError::Syntax);
    }
    fn fix_identifiers(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        return Err(ValidityError::Identifiers);
    }
    fn fix_calls(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        return Err(ValidityError::Calls);
    }
    fn fix_repetition(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> Result<(), ValidityError> {
        return Err(ValidityError::Repetition);
    }
}
pub trait Type {
    fn keyword() -> Keyword;
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Keyword {
    Require,
    RequireThen,
    Ensure,
    EnsureElse,
    Invariant,
}
impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self {
            Keyword::Require => "require",
            Keyword::RequireThen => "require then",
            Keyword::Ensure => "ensure",
            Keyword::EnsureElse => "ensure else",
            Keyword::Invariant => "invariant",
        };
        write!(f, "{}", content)
    }
}
