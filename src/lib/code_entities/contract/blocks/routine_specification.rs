use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;

use super::clause::Clause;
use super::*;

mod postcondition;
mod precondition;
pub use postcondition::Postcondition;
pub use precondition::Precondition;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize, JsonSchema, Default)]
#[schemars(deny_unknown_fields)]
#[schemars(
    description = "Hoare-style specifications of a given feature as preconditions and postconditions for AutoProof, Eiffel's static verifier."
)]
pub struct RoutineSpecification {
    pub precondition: Precondition,
    pub postcondition: Postcondition,
}

impl RoutineSpecification {
    pub fn is_empty(&self) -> bool {
        self.precondition.is_empty() && self.postcondition.is_empty()
    }
    pub fn from_markdown(markdown: &str) -> Self {
        let precondition: Precondition = markdown
            .lines()
            .skip_while(|line| !line.contains("# Pre"))
            .skip(1)
            .map_while(|line| {
                let line = line.trim();
                (!line.starts_with("# ")).then_some(Clause::from_line(line).or_else(|| {
                    info!("fail to parse the line:\t{line}\n");
                    None
                }))
            })
            .filter_map(|clause| clause)
            .collect::<Vec<_>>()
            .into();
        let postcondition: Postcondition = markdown
            .lines()
            .skip_while(|line| !line.contains("# Post"))
            .skip(1)
            .map_while(|line| {
                let line = line.trim();
                (!line.starts_with("# ")).then_some(Clause::from_line(line).or_else(|| {
                    info!("fail to parse the line:\t{line}\n");
                    None
                }))
            })
            .filter_map(|clause| clause)
            .collect::<Vec<_>>()
            .into();
        RoutineSpecification {
            precondition,
            postcondition,
        }
    }
}
