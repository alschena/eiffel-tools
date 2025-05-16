use crate::lib::code_entities::new_class::*;
use crate::lib::code_entities::new_class::*;
use crate::lib::code_entities::new_feature;
use crate::lib::code_entities::new_feature::*;
use std::fmt::Display;
use std::ops::Deref;

use super::code_entities::new_class;
use super::code_entities::prelude::ClassLocalModel;

struct EiffelCode(String);

impl EiffelCode {
    fn indent(text: &str) -> String {
        text.lines()
            .map(|line| format!("\t{line}\n"))
            .reduce(|acc, line| format!("{acc}{line}"))
            .unwrap_or_default()
    }

    fn model_code(model: &ClassLocalModel) -> String {
        model
            .names()
            .iter()
            .zip(model.types().iter())
            .map(|(name, _)| name.to_string())
            .reduce(|acc, name| format!("{acc}, {name}"))
            .map(|names_list| format!("model: {names_list}"))
            .unwrap_or_default()
    }

    fn parent_code(parents: &Parents, parent_name: &ClassName) -> String {
        let select = parents.selects(parent_name).into_iter().flatten().fold(
            String::new(),
            |mut acc, name| {
                if acc.is_empty() {
                    acc.push_str("select\n");
                }
                acc.push_str(&Self::indent(name));
                acc
            },
        );

        let redefine = parents.redefines(parent_name).into_iter().flatten().fold(
            String::new(),
            |mut acc, name| {
                if acc.is_empty() {
                    acc.push_str("redefine\n");
                }
                acc.push_str(&Self::indent(name));
                acc
            },
        );

        let undefine = parents.undefines(parent_name).into_iter().flatten().fold(
            String::new(),
            |mut acc, name| {
                if acc.is_empty() {
                    acc.push_str("undefine\n");
                }
                acc.push_str(&Self::indent(name));
                acc
            },
        );

        let rename = parents.rename_maps(parent_name).into_iter().flatten().fold(
            String::new(),
            |mut acc, (old_name, new_name)| {
                if acc.is_empty() {
                    acc.push_str("rename\n");
                }
                acc.push_str(&Self::indent(
                    format!("{} as {}", old_name.as_str(), new_name.as_str()).as_str(),
                ));
                acc
            },
        );

        let mut properties_text = format!("{}{}{}{}", &select, &redefine, &undefine, &rename);

        if !properties_text.is_empty() {
            properties_text.push_str("\nend");
        }

        format!(
            "{}\n{}",
            parent_name.as_str(),
            Self::indent(&properties_text)
        )
    }

    fn inheritance_code(parents: &Parents) -> String {
        let format_with_prefix_line = |parents_names: &Vec<_>, prefix_line: &str| {
            parents_names
                .into_iter()
                .map(|parent_name| Self::parent_code(parents, parent_name))
                .fold(String::new(), |mut acc, parent_code| {
                    if acc.is_empty() {
                        acc.push_str(prefix_line);
                    }
                    acc.push_str(&Self::indent(&parent_code));
                    acc
                })
        };

        let conformant_parents = format_with_prefix_line(parents.names_conformant(), "inherit\n");
        let nonconformant_parents =
            format_with_prefix_line(parents.names_nonconformant(), "inherit {NONE}\n");

        format!("{}{}", conformant_parents, nonconformant_parents)
    }

    fn feature_code(
        base_classes: &Classes,
        base_features: &Features,
        class_id: ClassID,
        feature_id: FeatureID,
        feature_body: String,
    ) -> String {
        debug_assert!(
            new_class::all_features(base_classes, base_features, class_id).contains(&feature_id)
        );
        let feature_name =
            new_class::feature_name(base_classes, base_features, class_id, feature_id).as_str();

        let feature_parameters = base_features
            .parameters(feature_id)
            .iter()
            .flat_map(|params| params.names().iter().zip(params.types().iter()))
            .fold(String::new(), |mut acc, (name, ty)| {
                if !acc.is_empty() {
                    acc.push_str("; ");
                }
                acc.push_str(format!("{}: {}", name, ty).as_str());
                acc
            });

        let feature_return_type = base_features
            .return_type(feature_id)
            .map(|ty| ty.to_string())
            .unwrap_or_default();

        format!(
            "{} ({}): {}\n\tdo\n{}\n\tend",
            feature_name,
            feature_parameters,
            feature_return_type,
            Self::indent(Self::indent(feature_body.as_str()).as_str())
        )
    }

    fn class_code<T: IntoIterator<Item = (FeatureID, String)>>(
        base_classes: &Classes,
        base_features: &Features,
        class_id: ClassID,
        features: T,
    ) -> String {
        let model: String = base_classes
            .model(class_id)
            .map(|model| Self::model_code(model))
            .unwrap_or_default();

        let name: String = base_classes.name(class_id).to_string();

        let inheritance: String = base_classes
            .parents(class_id)
            .map(|parents| Self::inheritance_code(parents))
            .unwrap_or_default();

        let features =
            features
                .into_iter()
                .fold(String::new(), |mut acc, (feature_id, feature_body)| {
                    if acc.is_empty() {
                        acc.push_str("features\n");
                    }
                    format!(
                        "{}{}\n",
                        acc,
                        Self::indent(&Self::feature_code(
                            base_classes,
                            base_features,
                            class_id,
                            feature_id,
                            feature_body
                        ))
                    )
                });

        format!("{model}\nclass\n\t{name}\n{inheritance}\nend")
    }

    fn subclass_redefining_features<N, B, F>(
        base_classes: &Classes,
        base_features: &Features,
        class_id: ClassID,
        new_name: N,
        features_to_redefine: F,
    ) -> String
    where
        N: AsRef<str>,
        B: ToString,
        F: AsRef<[(FeatureID, B)]>,
    {
        let parent_name = base_classes.name(class_id);

        let mut parents = Parents::default();
        let names_of_features_to_redefine: Vec<_> = features_to_redefine
            .as_ref()
            .iter()
            .map(|(id, _)| new_class::feature_name(base_classes, base_features, class_id, *id))
            .cloned()
            .collect();

        parents.add_conformant(parent_name.clone());
        parents.add_redefines(parent_name.clone(), names_of_features_to_redefine);

        let inheritance_block = Self::inheritance_code(&parents);

        let features_block = features_to_redefine
            .as_ref()
            .into_iter()
            .map(|(ft_id, ft_body)| {
                Self::feature_code(
                    base_classes,
                    base_features,
                    class_id,
                    *ft_id,
                    ft_body.to_string(),
                )
            })
            .fold(String::new(), |mut acc, ft_code| {
                if acc.is_empty() {
                    acc.push_str("feature\n");
                }
                acc.push_str(&Self::indent(&ft_code));
                acc
            });

        format!(
            "class\n{}\n{}\n{}\nend",
            Self::indent(new_name.as_ref()),
            inheritance_block,
            features_block
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::lib::code_entities::new_class;

    use super::*;

    fn line_by_line_eq<F: ToString, S: ToString>(left: F, right: S) -> bool {
        let binding = left.to_string();
        let left_trimmed_lines = binding
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty());

        let binding = right.to_string();
        let right_trimmed_lines = binding
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty());

        left_trimmed_lines
            .zip(right_trimmed_lines)
            .all(|(left, right)| left == right)
    }

    #[test]
    fn eiffel_source_inheritance() {
        let cls = Classes::mock_inheritance();
        let fts = Features::mock_inheritance();
        let class_id: ClassID = (0 as usize).into();

        let class_code = EiffelCode::class_code(&cls, &fts, class_id, []);

        let oracle = r#"
                class
                    CHILD
                inherit
                    PARENT
                        rename
                            parent_feature as child_feature
                        end
                end
            "#;

        assert!(
            line_by_line_eq(&class_code, &oracle),
            "result:\n{}\noracle:\n{}",
            class_code,
            oracle
        )
    }

    #[test]
    fn subclass_redefining_feature() {
        let cls = Classes::mock_singleton();
        let fts = Features::mock_singleton();

        let class_id = cls.id("TEST");
        let feature_id = new_class::all_features(&cls, &fts, class_id)
            .first()
            .copied()
            .unwrap();

        let subclass_code = EiffelCode::subclass_redefining_features(
            &cls,
            &fts,
            class_id,
            "TEST_INSTRUMENTED",
            vec![(feature_id, "Result := True -- TRIVIAL")],
        );

        let oracle = r#"
                class
                    TEST_INSTRUMENTED
                inherit
                    TEST
                        redefine
                            f
                        end
                feature
                    f (x: INTEGER): BOOLEAN
                        do
                            Result := True -- TRIVIAL
                        end
                end
            "#;

        assert!(
            line_by_line_eq(&subclass_code, &oracle),
            "result:\n{}\noracle:\n{}",
            subclass_code,
            oracle
        )
    }
}
