use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use contract::RoutineSpecification;
use std::sync::Arc;
use tracing::info;

mod constructor_api;
mod prompt;

use constructor_api::OpenAIResponseFormat;

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
        )?;
        // Generate json with specifications
        let completion_parameters = constructor_api::CompletionParameters {
            messages: prompt.to_llm_messages(),
            response_format: Some(OpenAIResponseFormat::json::<RoutineSpecification>()),
            n: Some(50),
            ..Default::default()
        };

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
                    serde_json::from_str::<RoutineSpecification>(candidate)
                        .map_err(|e| info!("fail to parse generated output with error: {e:#?}"))
                        .ok()
                })
            })
            .collect();

        Ok(completion_response_processed)
    }
}
