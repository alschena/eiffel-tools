use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::TextEdit;
use async_lsp::lsp_types::Url;
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

pub struct Generator {
    llm: constructor_api::LLM,
    prompt: prompt::Prompt,
}
impl Generator {
    pub async fn more_routine_specifications(
        &self,
        feature: &Feature,

        file: &ProcessedFile,
        system_classes: &[&Class],
    ) -> anyhow::Result<RoutineSpecification> {
        let current_class = file.class();
        let current_class_model = current_class
            .name()
            .model_extended(&system_classes)
            .unwrap_or_default();
        let url = Url::from_file_path(file.path()).expect("convert file path to url.");

        let prompt = prompt::Prompt::for_feature_specification(
            feature,
            &current_class_model,
            file,
            &system_classes,
        )?;
        let completion_parameters = prompt.to_completion_parameters();
        let completion_response = self.llm.model_complete(&completion_parameters).await?;

        // let mut routine_specification: RoutineSpecification =
        //     response.parse().map_err(|e| CodeActionDisabled {
        //         reason: format!("parse error {e:?}"),
        //     })?;

        // // Fix routine specification.
        // let corrected_responses = routine_specification
        //     .fix(&system_classes, current_class, feature)
        //     .then_some(routine_specification);

        // let spec = corrected_responses.ok_or_else(|| CodeActionDisabled {
        //     reason: "No added specification for routine was produced".to_string(),
        // })?;

        // Ok(WorkspaceEdit::new(HashMap::from([(
        //     url,
        //     vec![
        //         text_edit_add_precondition(
        //             &feature,
        //             feature.point_end_preconditions().unwrap().clone(),
        //             spec.precondition,
        //         ),
        //         text_edit_add_postcondition(
        //             &feature,
        //             feature.point_end_postconditions().unwrap().clone(),
        //             spec.postcondition,
        //         ),
        //     ],
        // )])))
        todo!()
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
