use async_lsp::lsp_types::CodeActionDisabled;

use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;

#[derive(Default)]
pub struct Prompt {
    text: String,
}

impl Prompt {
    pub fn into_string(self) -> String {
        self.text
    }
    pub fn append_preamble_text(&mut self) {
        self.text.push_str(
        "You are an expert in formal methods, specifically design by contract for static verification.\nRemember that model-based contract only refer to the model of the current class and the other classes referred by in the signature of the feature.\nYou are optionally adding model-based contracts to the following feature:\n"    
        )
    }
    pub fn append_feature_src_with_contract_holes(
        &mut self,
        feature: &Feature,
        file: &ProcessedFile,
    ) -> Result<(), CodeActionDisabled> {
        let Some(point_insert_preconditions) = feature.point_end_preconditions() else {
            return Err(CodeActionDisabled{
                reason:"Only attributes with an attribute block and routines support adding preconditions".to_string(),
            });
        };
        let Some(point_insert_postconditions) = feature.point_end_postconditions() else {
            return Err(CodeActionDisabled{reason:"Only attributes with an attribute block and routines support adding postconditions".to_string()});
        };
        let precondition_hole = if feature.has_precondition() {
            format!(
                "\n{}<ADD_PRECONDITION_CLAUSES>",
                contract::Precondition::indentation_string()
            )
        } else {
            format!(
                "<ADD_PRECONDITION_CLAUSES>\n{}",
                <contract::Block<contract::Precondition>>::indentation_string()
            )
        };
        let postcondition_hole = if feature.has_postcondition() {
            format!(
                "\n{}<ADD_POSTCONDITION_CLAUSES>",
                contract::Postcondition::indentation_string()
            )
        } else {
            format!(
                "<ADD_POSTCONDITION_CLAUSES>\n{}",
                <contract::Block<contract::Postcondition>>::indentation_string()
            )
        };
        let injections = vec![
            (point_insert_preconditions, precondition_hole.as_str()),
            (point_insert_postconditions, postcondition_hole.as_str()),
        ];
        let feature_src = file
            .feature_src_with_injections(&feature, injections.into_iter())
            .expect("inject feature source code");
        self.text.push_str("```eiffel\n");
        self.text.push_str(&feature_src);
        self.text.push_str("```\n");
        Ok(())
    }

    pub fn append_full_model_text(
        &mut self,
        feature: &Feature,
        class: &Class,
        system_classes: &[&Class],
    ) {
        let mut text = class
            .full_extended_model(&system_classes)
            .fmt_indented(ClassModel::INDENTATION_LEVEL);

        if text.is_empty() {
            text.push_str("The current class and its ancestors have no model.");
        } else {
            text.insert_str(0, "Models of the current class and its ancestors:\n{}");
        }

        let parameters = feature.parameters();
        let parameters_models_fmt = parameters
            .types()
            .iter()
            .map(|t| {
                t.class(system_classes.iter().copied())
                    .full_extended_model(&system_classes)
            })
            .map(|ext_model| ext_model.fmt_indented(ClassModel::INDENTATION_LEVEL));

        let parameters_models = parameters.names().iter().zip(parameters_models_fmt).fold(
            String::new(),
            |mut acc, (name, model_fmt)| {
                acc.push_str("Model of the argument ");
                acc.push_str(name);
                acc.push(':');
                acc.push('\n');
                acc.push_str(model_fmt.as_str());
                acc
            },
        );

        if !parameters_models.is_empty() {
            text.push_str(&parameters_models)
        }

        self.text.push_str(text.as_str());
        self.text.push('\n');
    }
}
