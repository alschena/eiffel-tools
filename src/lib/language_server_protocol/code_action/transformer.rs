use crate::lib::code_entities::prelude::*;
use anyhow::Result;
use contract::{Postcondition, Precondition};
use gemini;
use gemini::ToResponseSchema;
use rayon::iter::IntoParallelRefIterator;

pub struct LLM(gemini::Config);
impl LLM {
    fn config(&self) -> &gemini::Config {
        &self.0
    }
}
impl Default for LLM {
    fn default() -> Self {
        Self(gemini::Config::default())
    }
}
impl LLM {
    pub fn add_contracts(&self, routine_src: &str) -> Result<(Precondition, Postcondition)> {
        let mut request_precondition =
            gemini::Request::from(format!("Provide weakest preconditions"));
        request_precondition.set_config(gemini::GenerationConfig::from(
            Precondition::to_response_schema(),
        ));
        let mut request_postcondition =
            gemini::Request::from(format!("Provide strongest postconditions"));
        request_postcondition.set_config(gemini::GenerationConfig::from(
            Postcondition::to_response_schema(),
        ));
        let client = gemini::Request::new_blocking_client();
        let (precondition_response, postcondition_response) = (
            request_precondition.process_with_blocking_client(&self.config(), &client)?,
            request_postcondition.process_with_blocking_client(&self.config(), &client)?,
        );
        let mut all_pre = precondition_response
            .parsable_content()
            .map(|x| serde_json::from_str::<Precondition>(x));
        let mut all_post = postcondition_response
            .parsable_content()
            .map(|x| serde_json::from_str::<Postcondition>(x));
        // Select the first output for both the pre and post conditions agents.
        let pre = match all_pre.next() {
            Some(p) => p?,
            None => Precondition::from(Vec::new()),
        };
        let post = match all_post.next() {
            Some(p) => p?,
            None => Postcondition::from(Vec::new()),
        };
        Ok((pre, post))
    }
}
