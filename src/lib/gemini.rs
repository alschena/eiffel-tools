//! The content of the current conversation with the model.
//! For single-turn queries, this is a single instance.
//! For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
use crate::lib::code_entities;
use serde::{Deserialize, Serialize};
const DESCRIPTION_PRECONDITION: &str = "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.";

const DESCRIPTION_POSTCONDITION: &str = "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.
        ";
const DESCRIPTION_PRECONDITION_CLAUSE: &str =
    "Write a valid precondition clause for the Eiffel programming language.";
const DESCRIPTION_POSTCONDITION_CLAUSE: &str =
    "Write a valid postcondition clause for the Eiffel programming language.";
const DESCRIPTION_TAG_CLAUSE: &str =
    "Write a valid tag clause for the Eiffel programming language.";

#[derive(Deserialize, Serialize)]
struct PostconditionClause {
    tag: String,
    predicate: String,
}
impl Default for PostconditionClause {
    fn default() -> Self {
        let tag = "trivial".to_string();
        let predicate = "True".to_string();
        PostconditionClause { tag, predicate }
    }
}
impl From<PostconditionClause> for code_entities::ContractClause<code_entities::Postcondition> {
    fn from(value: PostconditionClause) -> Self {
        let tag = if value.tag.is_empty() {
            None
        } else {
            Some(code_entities::Tag::from(value.tag))
        };
        Self::new(tag, code_entities::Predicate::from(value.predicate))
    }
}
#[derive(Deserialize, Serialize)]
struct PreconditionClause {
    tag: String,
    predicate: String,
}
impl Default for PreconditionClause {
    fn default() -> Self {
        let tag = "trivial".to_string();
        let predicate = "True".to_string();
        PreconditionClause { tag, predicate }
    }
}
impl From<PreconditionClause> for code_entities::ContractClause<code_entities::Precondition> {
    fn from(value: PreconditionClause) -> Self {
        let tag = if value.tag.is_empty() {
            None
        } else {
            Some(code_entities::Tag::from(value.tag))
        };
        Self::new(tag, code_entities::Predicate::from(value.predicate))
    }
}
#[derive(Deserialize, Serialize)]
struct Precondition {
    precondition: Vec<PreconditionClause>,
}
impl From<Precondition> for code_entities::Contract<code_entities::Precondition> {
    fn from(value: Precondition) -> Self {
        let pre: Vec<code_entities::ContractClause<code_entities::Precondition>> = value
            .precondition
            .into_iter()
            .map(|x| code_entities::ContractClause::<code_entities::Precondition>::from(x))
            .collect();
        Self::from(pre)
    }
}
#[derive(Deserialize, Serialize)]
struct Postcondition {
    postcondition: Vec<PostconditionClause>,
}
impl From<Postcondition> for code_entities::Contract<code_entities::Postcondition> {
    fn from(value: Postcondition) -> Self {
        let post: Vec<code_entities::ContractClause<code_entities::Postcondition>> = value
            .postcondition
            .into_iter()
            .map(|x| code_entities::ContractClause::<code_entities::Postcondition>::from(x))
            .collect();
        Self::from(post)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_contract() {
        let pre_str = r#"{
  "precondition": [
    {
      "tag": "name_pre",
      "predicate": "a = b"
    }
  ]
}"#;
        let post_str = r#"{
  "postcondition": [
    {
      "tag": "name_post",
      "predicate": "a = b"
    }
  ]
}"#;
        let pre: Precondition = serde_json::from_str(pre_str).expect("Parse precondition");
        let post: Postcondition = serde_json::from_str(post_str).expect("Parse postcondition");
    }
}
