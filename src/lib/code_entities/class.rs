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
    features: Vec<Feature>,
    descendants: Vec<Class>,
    ancestors: Vec<Class>,
    range: Range,
}

impl Class {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
    pub fn features(&self) -> &Vec<Feature> {
        &self.features
    }
    pub fn into_features(self) -> Vec<Feature> {
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
        self.features.push(feature.clone())
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
impl TryFrom<lsp_types::DocumentSymbol> for Class {
    type Error = anyhow::Error;

    fn try_from(value: lsp_types::DocumentSymbol) -> std::result::Result<Self, Self::Error> {
        let name = value.name;
        let kind = value.kind;
        let range = value.range.try_into()?;
        debug_assert_eq!(kind, lsp_types::SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v
                .into_iter()
                .map(|x| Feature::try_from(x).expect("Document symbol to feature"))
                .collect(),
            None => Vec::new(),
        };
        Ok(Class::from_name_range(name, range))
    }
}
impl TryFrom<&Class> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let features = value.features();
        let range = value.range().clone().try_into()?;
        let children: Option<Vec<lsp_types::DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| x.try_into().expect("feature conversion to document symbol"))
                .collect(),
        );
        Ok(lsp_types::DocumentSymbol {
            name,
            detail: None,
            kind: lsp_types::SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children,
        })
    }
}
impl<'a> TryFrom<(&tree_sitter::Tree, &'a str)> for Class {
    type Error = anyhow::Error;

    fn try_from((tree, src): (&tree_sitter::Tree, &'a str)) -> Result<Self, Self::Error> {
        let mut cursor = tree.walk();
        let mut traversal = tree_sitter::WidthFirstTraversal::new(&mut cursor);

        // Extract class name
        let node = traversal
            .find(|x| x.kind() == "class_name")
            .expect("class_name");

        let name = src[node.byte_range()].into();
        let range = node.range().into();
        let mut class = Self::from_name_range(name, range);

        // Extract features
        let features: Vec<Feature> = traversal
            .filter(|x| x.kind() == "feature_declaration")
            .map(|node| Feature::try_from((&node, tree.walk(), src)))
            .collect::<anyhow::Result<Vec<Feature>>>()?;
        class.features = features;

        // Extract optional model
        let mut model_names: Vec<String> = Vec::new();
        let mut cursor = tree.walk();
        let tag = tree_sitter::WidthFirstTraversal::new(&mut cursor).find(|x| {
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
                f
            })
            .collect();
        let model = Model(model);
        class.add_model(&model);
        Ok(class)
    }
}
impl TryFrom<&Class> for lsp_types::WorkspaceSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let features = value.features();
        let children: Option<Vec<lsp_types::DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| lsp_types::DocumentSymbol::try_from(x))
                .collect::<anyhow::Result<Vec<lsp_types::DocumentSymbol>>>()?,
        );
        let location = match value.location() {
            Some(v) => v.try_into()?,
            None => anyhow::bail!("Expected class with valid file location"),
        };
        Ok(lsp_types::WorkspaceSymbol {
            name,
            kind: lsp_types::SymbolKind::CLASS,
            tags: None,
            container_name: None,
            location: lsp_types::OneOf::Right(location),
            data: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::processed_file;
    use ::tree_sitter;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

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
    #[test]
    fn class_to_workspacesymbol() {
        let path = "/tmp/eiffel_tool_test_class_to_workspacesymbol.e";
        let path = PathBuf::from(path);
        let src = "
    class A
    note
    end
        ";
        let mut file = File::create(path.clone()).expect("Failed to create file");
        file.write_all(src.as_bytes())
            .expect("Failed to write to file");
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");
        let file = processed_file::ProcessedFile::new(&mut parser, path.clone());
        let class: Class = (&file).try_into().expect("Parse class");
        let symbol = <lsp_types::WorkspaceSymbol>::try_from(&class);
        assert!(symbol.is_ok())
    }
}
