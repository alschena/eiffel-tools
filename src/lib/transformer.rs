use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::TextEdit;
use contract::{Postcondition, Precondition, RoutineSpecification};

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
        let completion_parameters = prompt.to_completion_parameters();
        let completion_response = self.llm.model_complete(&completion_parameters).await?;

        Ok(completion_response
            .contents()
            .map(|c| RoutineSpecification::from_markdown(c))
            .collect())
    }
}
fn text_edit_add_postcondition(
    feature: &Feature,
    point: Point,
    postcondition: Postcondition,
) -> TextEdit {
    let postcondition_text = if feature.has_postcondition() {
        format!("{postcondition}")
    } else {
        format!(
            "{}",
            contract::Block::<contract::Postcondition>::new(
                postcondition,
                Range::new_collapsed(point.clone())
            )
        )
    };
    TextEdit {
        range: Range::new_collapsed(point)
            .try_into()
            .expect("range should convert to lsp-type range."),
        new_text: postcondition_text,
    }
}
fn text_edit_add_precondition(
    feature: &Feature,
    point: Point,
    precondition: Precondition,
) -> TextEdit {
    let precondition_text = if feature.has_precondition() {
        format!("{precondition}")
    } else {
        format!(
            "{}",
            contract::Block::<contract::Precondition>::new(
                precondition,
                Range::new_collapsed(point.clone())
            )
        )
    };
    TextEdit {
        range: Range::new_collapsed(point)
            .try_into()
            .expect("range should convert to lsp-type range."),
        new_text: precondition_text,
    }
}
