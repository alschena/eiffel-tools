use super::prelude::*;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::DerefMut;
use tracing::info;

mod blocks;
pub use blocks::Block;
pub use blocks::Postcondition;
pub use blocks::Precondition;
pub use blocks::RoutineSpecification;

mod clause;
pub use clause::Clause;
pub use clause::Predicate as ClausePredicate;
pub use clause::Tag as ClauseTag;

pub trait Contract: DerefMut<Target = Vec<Clause>> {
    fn keyword() -> Keyword;
    fn remove_self_redundant_clauses(&mut self) {
        let mut remove = self
            .iter()
            .enumerate()
            .map(|(n, c)| {
                self.iter()
                    .skip(n + 1)
                    .any(|nc| nc.predicate == c.predicate)
            })
            .collect::<Vec<bool>>()
            .into_iter();

        self.retain(|_| !remove.next().expect("`keep` has the same count as `self`."));
    }
    fn remove_redundant_clauses(&mut self, block: &Self) {
        self.remove_self_redundant_clauses();
        self.retain(|clause| block.iter().all(|c| c.predicate != clause.predicate));
    }
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
