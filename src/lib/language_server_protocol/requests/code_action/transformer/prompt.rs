use async_lsp::lsp_types::CodeActionDisabled;

use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;

pub struct Prompt {
    preable: String,
    source_with_holes: String,
    full_model: String,
}

impl Default for Prompt {
    fn default() -> Self {
        Self { preable: (String::from("You are an expert in formal methods, specifically design by contract for static verification.\nRemember that model-based contract only refer to the model of the current class and the other classes referred by in the signature of the feature.\nYou are optionally adding model-based contracts to the following feature:\n")), source_with_holes: String::new(), full_model: String::new() }
    }
}

impl Prompt {
    pub fn for_feature_specification(
        feature: &Feature,
        class_model: &ClassModel,
        file: &ProcessedFile,
        system_classes: &[&Class],
    ) -> Result<Self, CodeActionDisabled> {
        let mut var = Self::default();
        var.set_feature_src_with_contract_holes(feature, file)?;
        var.set_full_model_text(feature.parameters(), class_model, system_classes);
        Ok(var)
    }
    pub fn text(&self) -> String {
        let mut text = String::new();
        text.push_str(&self.preable);
        text.push_str(&self.source_with_holes);
        text.push_str(&self.full_model);
        text
    }
    pub fn set_preable(&mut self, preable: &str) {
        self.preable.clear();
        self.preable.push_str(preable);
    }
    pub fn set_feature_src_with_contract_holes(
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

        self.source_with_holes.clear();
        self.source_with_holes.push_str("```eiffel\n");
        self.source_with_holes.push_str(&feature_src);
        self.source_with_holes.push_str("```\n");
        Ok(())
    }

    pub fn set_full_model_text(
        &mut self,
        feature_parameters: &FeatureParameters,
        class_model: &ClassModel,
        system_classes: &[&Class],
    ) {
        let mut text = class_model.fmt_indented(ClassModel::INDENTATION_LEVEL);

        if text.is_empty() {
            text.push_str("The current class and its ancestors have no model.\n");
        } else {
            text.insert_str(0, "Models of the current class and its ancestors:\n{}");
        }

        let parameters_models_fmt = feature_parameters
            .types()
            .iter()
            .map(|t| {
                t.class(system_classes.iter().copied())
                    .full_extended_model(&system_classes)
            })
            .map(|ext_model| ext_model.fmt_indented(ClassModel::INDENTATION_LEVEL));

        let parameters_models = feature_parameters
            .names()
            .iter()
            .zip(parameters_models_fmt)
            .fold(String::new(), |mut acc, (name, model_fmt)| {
                acc.push_str("Model of the argument ");
                acc.push_str(name);
                acc.push(':');
                acc.push('\n');
                acc.push_str(model_fmt.as_str());
                acc
            });

        if !parameters_models.is_empty() {
            text.push_str(&parameters_models)
        }

        self.full_model.clear();
        self.full_model.push_str(text.as_str());
        self.full_model.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    fn parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        parser
    }

    #[tokio::test]
    async fn prompt_boxed_integer_arg() {
        let mut parser = parser();
        let temp_dir = TempDir::new().expect("must create temporary directory.");
        let src = r#"
class A feature
  x (arg: NEW_INTEGER)
    do
    end
end
        "#;
        let file = temp_dir.child("test_prompt.e");
        file.write_str(src).expect("temp file must be writable");

        assert!(file.exists());

        let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("processed file must be produced.");

        let class = processed_file.class();
        let src_supplier = r#"note
	model: value
class
	NEW_INTEGER
feature
	value: INTEGER
	smaller (other: NEW_INTEGER): BOOLEAN
		do
			Result := value < other.value
		ensure
			Result = (value < other.value)
		end
end
    "#;
        let supplier = Class::from_source(&src_supplier);

        let system_classes = vec![class, &supplier];
        let class_model = class.full_extended_model(&system_classes);

        let feature = class.features().first().expect("first features is `x`");
        let feature_parameters = feature.parameters();

        let mut prompt = Prompt::default();
        prompt
            .set_feature_src_with_contract_holes(feature, &processed_file)
            .expect("set feature src with contract holes.");
        prompt.set_full_model_text(feature_parameters, &class_model, &system_classes);

        eprintln!("{}", prompt.text());
        assert_eq!(
            prompt.text(),
            r#"You are an expert in formal methods, specifically design by contract for static verification.
Remember that model-based contract only refer to the model of the current class and the other classes referred by in the signature of the feature.
You are optionally adding model-based contracts to the following feature:
```eiffel
  x (arg: NEW_INTEGER)
    <ADD_PRECONDITION_CLAUSES>
		do
    <ADD_POSTCONDITION_CLAUSES>
		end
```
The current class and its ancestors have no model.
Model of the argument arg:
	value: INTEGER
"#
        );
    }

    // #[tokio::test]
    //     async fn prompt() {
    //         let mut parser = parser();
    //         let temp_dir = TempDir::new().expect("must create temporary directory.");
    //         let src = r#"
    // class A feature
    //   x (arg: NEW_INTEGER): NEW_INTEGER
    //     do
    //     end
    // end
    //         "#;
    //         let file = temp_dir.child("test_prompt.e");
    //         file.write_str(src).expect("temp file must be writable");

    //         assert!(file.exists());

    //         let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
    //             .await
    //             .expect("processed file must be produced.");

    //         let class = processed_file.class();
    //         let src_supplier = r#"note
    // 	model: value
    // class
    // 	NEW_INTEGER
    // feature
    // 	value: INTEGER
    // 	smaller (other: NEW_INTEGER): BOOLEAN
    // 		do
    // 			Result := value < other.value
    // 		ensure
    // 			Result = (value < other.value)
    // 		end
    // end
    //     "#;
    //         let supplier = Class::from_source(&src_supplier);

    //         let system_classes = vec![class, &supplier];
    //         let class_model = class.full_extended_model(&system_classes);

    //         let feature = class.features().first().expect("first features is `x`");
    //         let feature_parameters = feature.parameters();

    //         let mut prompt = Prompt::default();
    //         prompt
    //             .set_feature_src_with_contract_holes(feature, &processed_file)
    //             .expect("set feature src with contract holes.");
    //         prompt.set_full_model_text(feature_parameters, &class_model, &system_classes);

    //         eprintln!("{}", prompt.text());
    //         assert_eq!(
    //             prompt.text(),
    //             r#"You are an expert in formal methods, specifically design by contract for static verification.
    // Remember that model-based contract only refer to the model of the current class and the other classes referred by in the signature of the feature.
    // You are optionally adding model-based contracts to the following feature:
    // ```eiffel
    //   x (arg: NEW_INTEGER): NEW_INTEGER
    //     <ADD_PRECONDITION_CLAUSES>
    // 		do
    //     <ADD_POSTCONDITION_CLAUSES>
    // 		end
    // ```
    // The current class and its ancestors have no model.
    // Model of the argument arg:
    // 	value: INTEGER
    // "#
    //         );
    //     }
}
