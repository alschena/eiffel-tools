use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use constructor_api::OpenAIResponseFormat;
use contract::RoutineSpecification;
use schemars::schema_for;
use tracing::info;

mod prompt;

#[cfg(feature = "constructor")]
mod constructor_api;

#[cfg(feature = "gemini")]
mod gemini;
#[cfg(feature = "gemini")]
pub use gemini::LLM;

#[cfg(feature = "ollama")]
mod ollama;
#[cfg(feature = "ollama")]
pub use ollama::LLM;
use prompt::Prompt;

pub struct Generator {
    llm: constructor_api::LLM,
    prompt: prompt::Prompt,
}
impl Generator {
    pub async fn try_new() -> anyhow::Result<Self> {
        let llm = constructor_api::LLM::try_new().await?;
        let prompt = Prompt::default();
        Ok(Self { llm, prompt })
    }
    pub async fn more_routine_specifications(
        &self,
        feature: &Feature,

        file: &ProcessedFile,
        system_classes: &[&Class],
    ) -> anyhow::Result<Vec<RoutineSpecification>> {
        let current_class = file.class();
        let current_class_model = current_class
            .name()
            .model_extended(&system_classes)
            .unwrap_or_default();

        let prompt = prompt::Prompt::for_feature_specification(
            feature,
            &current_class_model,
            file,
            &system_classes,
        )?;
        let completion_parameters = constructor_api::CompletionParameters {
            messages: prompt.to_llm_messages(),
            response_format: Some(OpenAIResponseFormat::json_schema::<RoutineSpecification>()),
            ..Default::default()
        };
        let completion_response = self.llm.model_complete(&completion_parameters).await?;

        Ok(completion_response
            .contents()
            .filter_map(|c| {
                serde_json::from_str(c)
                    .map_err(|e| info!("fail to parse generated output with error: {e:#?}"))
                    .ok()
            })
            .collect())
    }
}
