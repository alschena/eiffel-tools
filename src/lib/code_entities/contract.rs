use super::prelude::*;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::DerefMut;

mod blocks;
pub use blocks::Block;
pub use blocks::Postcondition;
pub use blocks::Precondition;
pub use blocks::RoutineSpecification;

mod clause;
use clause::Clause;
use tracing::info;

pub(crate) trait Fix: Debug {
    fn fix(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        if !self.fix_syntax(system_classes, current_class, current_feature) {
            info!(target:"llm", "fail fix syntax {self:?}");
            return false;
        }

        if !self.fix_identifiers(system_classes, current_class, current_feature) {
            info!(target:"llm", "fail fix identifiers");
            return false;
        }

        if !self.fix_calls(system_classes, current_class, current_feature) {
            info!(target:"llm", "fail fix calls");
            return false;
        }

        if !self.fix_repetition(system_classes, current_class, current_feature) {
            info!(target:"llm", "fail fix repetition");
            return false;
        }
        true
    }
    fn fix_syntax(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
    fn fix_identifiers(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
    fn fix_calls(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
    fn fix_repetition(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
}

pub trait Contract: DerefMut<Target = Vec<Clause>> {
    fn keyword() -> Keyword;
    fn remove_self_redundant_clauses(&mut self) {
        let mut remove = self
            .iter()
            .enumerate()
            .map(|(n, c)| {
                self.iter()
                    .skip(n + 1)
                    .any(|nc| &nc.predicate == &c.predicate)
            })
            .collect::<Vec<bool>>()
            .into_iter();

        self.retain(|_| !remove.next().expect("`keep` has the same count as `self`."));
    }
    fn remove_redundant_clauses(&mut self, block: &Self) {
        self.remove_self_redundant_clauses();
        self.retain(|clause| block.iter().all(|c| &c.predicate != &clause.predicate));
    }
}
impl<T: Contract> Indent for T {
    const INDENTATION_LEVEL: usize = 3;
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
