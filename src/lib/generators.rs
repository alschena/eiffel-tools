use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use crate::lib::tree_sitter_extension::Parse;
use contract::RoutineSpecification;
use std::sync::Arc;
use tracing::info;

mod post_processing;
mod prompt;

mod constructor_api;

#[derive(Debug, Default)]
pub struct Generators {
    llms: Vec<Arc<constructor_api::LLM>>,
}
impl Generators {
    pub async fn add_new(&mut self) {
        let Ok(llm) = constructor_api::LLM::try_new().await else {
            info!("fail to create LLM via constructor API");
            return;
        };
        self.llms.push(Arc::new(llm));
    }
    pub async fn more_routine_specifications(
        &self,
        feature: &Feature,
        file: &ProcessedFile,
        system_classes: &[Class],
    ) -> anyhow::Result<Vec<RoutineSpecification>> {
        let current_class = file.class();
        let current_class_model = current_class
            .name()
            .model_extended(&system_classes)
            .unwrap_or_default();

        let prompt = prompt::Prompt::for_feature_specification(
            feature,
            &current_class_model,
            file.path(),
            &system_classes,
        )
        .await?;
        // Generate feature with specifications
        let completion_parameters = constructor_api::CompletionParameters {
            messages: prompt.to_llm_messages_code_output(),
            n: Some(50),
            ..Default::default()
        };

        info!("{completion_parameters:#?}");

        let mut tasks = tokio::task::JoinSet::new();
        for llm in self.llms.iter().cloned() {
            let completion_parameters = completion_parameters.clone();
            tasks.spawn(async move { llm.model_complete(&completion_parameters).await });
        }
        let completion_response = tasks.join_all().await;

        let completion_response_processed = completion_response
            .iter()
            .filter_map(|rs| match rs {
                Err(e) => {
                    info!("An LLM request has returned the error: {e:#?}");
                    None
                }
                Ok(reply) => Some(reply),
            })
            .flat_map(|reply| {
                reply.contents().filter_map(|candidate| {
                    info!("candidate:\t{candidate}");
                    <RoutineSpecification as Parse>::parse(candidate)
                        .map_err(|e| info!("fail to parse generated output with error: {e:#?}"))
                        .ok()
                })
            })
            .collect();
        info!("completions:\t{completion_response_processed:#?}");

        Ok(completion_response_processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::generators::constructor_api::CompletionParameters;
    use crate::lib::generators::constructor_api::MessageOut;
    use crate::lib::generators::constructor_api::LLM;
    use crate::lib::tree_sitter_extension::Parse;

    #[ignore]
    #[tokio::test]
    async fn extract_from_code_output() -> anyhow::Result<()> {
        let llm = LLM::try_new().await?;
        let system_message_content = r#"You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code.            
"#;
        let user_message_content = r#"-- For the current class and its ancestors, the model is value: INTEGER
-- the model is implemented in Boogie.
-- For the argument other: NEW_INTEGER
--  the model is value: INTEGER
--    the model is terminal, no qualified call on it is allowed.
smaller (other: NEW_INTEGER): BOOLEAN
	do
		Result := value < other.value
	ensure
		Result = (value < other.value)
	end
	
"#;
        let messages = vec![
            MessageOut::new_system(system_message_content.to_string()),
            MessageOut::new_user(user_message_content.to_string()),
        ];
        let llm_parameters = CompletionParameters {
            messages,
            model: constructor_api::EnumLanguageModel::GeminiFlash,
            ..Default::default()
        };
        let output = llm.model_complete(&llm_parameters).await?;
        let specs: Vec<RoutineSpecification> = output
            .extract_multiline_code()
            .into_iter()
            .inspect(|code| eprintln!("{code}"))
            .map(|code| {
                Feature::parse(&code).expect("parsing must succed (possibly with error nodes).")
            })
            .map(|ft| ft.routine_specification())
            .inspect(|spec| eprintln!("{spec:#?}"))
            .collect();
        Ok(())
    }
}
