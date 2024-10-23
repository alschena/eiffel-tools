use crate::lib::code_entities::prelude::*;
use gemini;

pub struct LLM(gemini::model::Config);
impl Default for LLM {
    fn default() -> Self {
        Self(gemini::model::Config::default())
    }
}
impl LLM {
    pub fn add_contracts(
        &self,
        routine: &Feature,
    ) -> (contract::Precondition, contract::Postcondition) {
        (
            contract::Precondition {
                precondition: vec![contract::Clause {
                    tag: contract::Tag {
                        tag: "test_precondition".to_string(),
                    },
                    predicate: contract::Predicate {
                        predicate: "True".to_string(),
                    },
                }],
            },
            contract::Postcondition {
                postcondition: vec![contract::Clause {
                    tag: contract::Tag {
                        tag: "test_postcondition".to_string(),
                    },
                    predicate: contract::Predicate {
                        predicate: "True".to_string(),
                    },
                }],
            },
        )
    }
}
