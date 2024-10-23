use crate::lib::code_entities::prelude as code_entities;
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
        routine: &code_entities::Feature,
    ) -> (code_entities::Precondition, code_entities::Postcondition) {
        (
            code_entities::Precondition {
                precondition: vec![code_entities::ContractClause {
                    tag: code_entities::Tag {
                        tag: "Test".to_string(),
                    },
                    predicate: code_entities::Predicate {
                        predicate: "True".to_string(),
                    },
                }],
            },
            code_entities::Postcondition {
                postcondition: vec![code_entities::ContractClause {
                    tag: code_entities::Tag {
                        tag: "Test".to_string(),
                    },
                    predicate: code_entities::Predicate {
                        predicate: "True".to_string(),
                    },
                }],
            },
        )
    }
}
