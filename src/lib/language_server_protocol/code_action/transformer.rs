use crate::lib::code_entities;
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
        todo!()
    }
}
