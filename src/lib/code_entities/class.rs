use super::feature::Feature;
use super::shared::*;
use crate::lib::tree_sitter;
use async_lsp::lsp_types;
use std::path::PathBuf;
// TODO accept only attributes of logical type in the model
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Model(pub Vec<Feature>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Class {
    name: String,
    path: Option<Location>,
    model: Model,
    features: Vec<Box<Feature>>,
    descendants: Vec<Box<Class>>,
    ancestors: Vec<Box<Class>>,
    range: Range,
}

impl Class {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
    pub fn features(&self) -> &Vec<Box<Feature>> {
        &self.features
    }
    pub fn into_features(self) -> Vec<Box<Feature>> {
        self.features
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn location(&self) -> Option<&Location> {
        match &self.path {
            None => None,
            Some(file) => Some(&file),
        }
    }
    pub fn from_name_range(name: String, range: Range) -> Class {
        let model = Model(Vec::new());
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            path: None,
            model,
            features,
            descendants,
            ancestors,
            range,
        }
    }

    pub fn add_feature(&mut self, feature: &Feature) {
        self.features.push(Box::new(feature.clone()))
    }

    pub fn add_model(&mut self, model: &Model) {
        self.model = model.clone()
    }

    pub fn add_location(&mut self, path: &PathBuf) {
        let path = path.clone();
        self.path = Some(Location { path })
    }
}
impl TryFrom<&Class> for lsp_types::Location {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let range = value.range().clone().try_into()?;
        let uri = value
            .location()
            .expect("Valid location of class")
            .try_into()
            .expect("Extraction of location from class");
        Ok(Self { uri, range })
    }
}
impl TryFrom<&Class> for lsp_types::SymbolInformation {
    type Error = anyhow::Error;
    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().into();
        let kind = lsp_types::SymbolKind::CLASS;
        let tags = None;
        let deprecated = None;
        let container_name = None;
        match value.try_into() {
            Err(e) => Err(e),
            Ok(location) => Ok(Self {
                name,
                kind,
                tags,
                deprecated,
                location,
                container_name,
            }),
        }
    }
}
impl<'a> TryFrom<(&tree_sitter::Tree, &'a str)> for Class {
    type Error = anyhow::Error;

    fn try_from((tree, src): (&tree_sitter::Tree, &'a str)) -> Result<Self, Self::Error> {
        let mut traversal = tree_sitter::WidthFirstTraversal::new(tree.walk());

        // Extract class name
        let node = traversal
            .find(|x| x.kind() == "class_name")
            .expect("class_name");

        let name = src[node.byte_range()].into();
        let range = node.range().into();
        let mut class = Self::from_name_range(name, range);

        // Extract features
        for node in traversal.filter(|x| x.kind() == "feature_declaration") {
            let range = node.range().into();
            let mut cursor = tree.walk();
            cursor.reset(node);
            let mut traversal = tree_sitter::WidthFirstTraversal::new(cursor);
            let name = src[traversal
                .find(|x| x.kind() == "extended_feature_name")
                .expect("Each feature declaration contains an extended feature name")
                .byte_range()]
            .into();
            let feature = Feature::from_name_and_range(name, range);
            class.add_feature(&feature);
        }

        // Extract optional model
        let mut model_names: Vec<String> = Vec::new();
        let tag = tree_sitter::WidthFirstTraversal::new(tree.walk()).find(|x| {
            x.kind() == "tag"
                && &src[x.byte_range()] == "model"
                && x.parent().is_some_and(|p| {
                    p.kind() == "note_entry"
                        && p.parent().is_some_and(|pp| {
                            pp.parent()
                                .is_some_and(|ppp| ppp.kind() == "class_declaration")
                        })
                })
        });
        match tag {
            Some(n) => {
                let mut next = n.next_sibling();
                while next.is_some() {
                    let current = next.unwrap();
                    model_names.push(src[current.byte_range()].to_string());
                    next = current.next_sibling();
                }
            }
            None => {}
        }
        let features_of_current_class = class.features();
        let model: Vec<Feature> = model_names
            .iter()
            .filter(|x| {
                features_of_current_class
                    .iter()
                    .find(|&y| y.name() == x.as_str())
                    .is_some()
            })
            .map(|x| {
                let f = features_of_current_class
                    .iter()
                    .find(|&y| y.name() == x.as_str())
                    .unwrap()
                    .clone();
                *f
            })
            .collect();
        let model = Model(model);
        class.add_model(&model);
        Ok(class)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ::tree_sitter;

    #[test]
    fn process_base_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
    class A
    note
    end
        ";
        let tree = parser.parse(src, None).expect("AST");

        let class = Class::try_from((&tree, src)).expect("Parse class");

        assert_eq!(
            class.name(),
            "A".to_string(),
            "Equality of {} and {}",
            class.name(),
            "A".to_string()
        );
    }

    #[test]
    fn process_annotated_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    ";
        let tree = parser.parse(src, None).expect("AST");

        let class = Class::try_from((&tree, src)).expect("Parse class");

        assert_eq!(class.name(), "DEMO_CLASS".to_string());
    }
    #[test]
    fn process_procedure() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";
        let tree = parser.parse(src, None).unwrap();
        let class = Class::try_from((&tree, src)).expect("Parse class");
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "f".to_string());
    }

    #[test]
    fn process_attribute() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
class A
feature
    x: INTEGER
end
";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::try_from((&tree, src)).expect("Parse class");
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "x".to_string());
    }
    #[test]
    fn process_model() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
note
    model: seq
class A
feature
    x: INTEGER
    seq: MML_SEQUENCE [INTEGER]
end
";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::try_from((&tree, src)).expect("Parse class");
        let model = class.model().clone();
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!((&model.0.first().unwrap()).name(), "seq".to_string());
        assert_eq!(features.first().unwrap().name(), "x".to_string());
    }
}
