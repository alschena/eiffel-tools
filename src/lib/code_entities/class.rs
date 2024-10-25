use super::prelude::*;
use crate::lib::tree_sitter_extension::{self, Node, Parse};
use anyhow::anyhow;
use async_lsp::lsp_types;
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tree_sitter::{Parser, Query, QueryCursor};
// TODO accept only attributes of logical type in the model
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Model(pub Vec<Feature>);
impl Model {
    fn new() -> Model {
        Model(Vec::new())
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
struct ModelNames(Vec<String>);
impl Parse for ModelNames {
    type Error = anyhow::Error;

    fn parse(root: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(root.parent().is_none());

        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let name_query = Query::new(
            lang,
            "(class_declaration (notes (note_entry (tag) @tag (identifier) @id)) \
               (#eq? @tag \"model\"))",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut matches = binding.matches(&name_query, root.clone(), src.as_bytes());

        let mut names: Vec<String> = Vec::new();
        while let Some(mat) = matches.next() {
            for n in mat.nodes_for_capture_index(1) {
                names.push(src[n.byte_range()].to_string())
            }
        }

        Ok(ModelNames(names))
    }
}
impl Model {
    fn from_model_names(names: ModelNames, features: &Vec<Feature>) -> Model {
        Model(
            names
                .0
                .iter()
                .filter_map(|name| {
                    features
                        .iter()
                        .find(|feature| feature.name() == name)
                        .cloned()
                })
                .collect(),
        )
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Class {
    name: String,
    path: Option<Location>,
    model: Model,
    features: Vec<Feature>,
    ancestors: Vec<Ancestor>,
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
    pub fn ancestors(&self) -> &Vec<Ancestor> {
        &self.ancestors
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
        Class {
            name,
            path: None,
            model: Model(Vec::new()),
            features: Vec::new(),
            ancestors: Vec::new(),
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
impl Indent for Class {
    const INDENTATION_LEVEL: u32 = 1;
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
impl Parse for Class {
    type Error = anyhow::Error;
    #[instrument(skip_all)]
    fn parse(root: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert!(root.parent().is_none());
        let mut cursor = QueryCursor::new();
        // Extract class name
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let name_query = Query::new(lang, "(class_declaration (class_name) @name)").unwrap();

        let mut captures = cursor.captures(&name_query, root.clone(), src.as_bytes());

        let name_node = match captures.next() {
            Some(v) => v.0.captures[0].node,
            None => return Err(anyhow!("fails to parse class name ")),
        };

        let name = src[name_node.byte_range()].into();
        let range = name_node.range().into();
        let mut class = Self::from_name_range(name, range);

        // Extract ancestors
        let ancestor_query = Query::new(lang, "(inheritance) @ancestors").map_err(|e| {
            anyhow!(
                "fails to query `(inheritance) @ancestors)))` with error: {:?}",
                e
            )
        })?;

        let mut inheritance_block = cursor.matches(&ancestor_query, root.clone(), src.as_bytes());

        let mut ancestors = Vec::new();
        while let Some(mat) = inheritance_block.next() {
            for cap in mat.captures {
                ancestors.append(&mut <Vec<Ancestor>>::parse(&cap.node, src)?)
            }
        }

        // Extract features
        let feature_query = Query::new(lang, "(feature_declaration) @dec").unwrap();

        let mut feature_cursor = cursor.matches(&feature_query, root.clone(), src.as_bytes());

        let mut features: Vec<Feature> = Vec::new();
        while let Some(mat) = feature_cursor.next() {
            for cap in mat.captures {
                features.push(Feature::parse(&cap.node, src)?);
            }
        }

        // Extract optional model
        class.model = Model::from_model_names(ModelNames::parse(root, src)?, &features);
        class.features = features;
        class.ancestors = ancestors;
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
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Ancestor {
    name: String,
    select: Vec<String>,
    rename: Vec<(String, String)>,
    redefine: Vec<String>,
    undefine: Vec<String>,
}
impl Ancestor {
    fn name(&self) -> &str {
        &self.name
    }
}
impl Parse for Vec<Ancestor> {
    type Error = anyhow::Error;

    #[instrument(skip_all)]
    fn parse(node: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "inheritance");
        let lang = &tree_sitter_eiffel::LANGUAGE.into();

        let ancestor_query = Query::new(
            lang,
            "(parent (class_type (class_name) @ancestor))",
        ).map_err(|e| anyhow!("fails to query `(parent (class_type (class_name) @ancestor))` with error: {:?}",e))?;

        let mut binding = QueryCursor::new();
        let mut matches = binding.matches(&ancestor_query, node.clone(), src.as_bytes());

        let mut ancestors = Vec::new();
        while let Some(mat) = matches.next() {
            for cap in mat.captures {
                let node = &cap.node;
                ancestors.push(Ancestor {
                    name: src[node.byte_range()].into(),
                    select: Vec::new(),
                    rename: Vec::new(),
                    redefine: Vec::new(),
                    undefine: Vec::new(),
                });
            }
        }
        Ok(ancestors)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::processed_file;
    use anyhow::Result;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;
    use tree_sitter;

    #[test]
    fn parse_base_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        let src = "
    class A
    note
    end
        ";
        let tree = parser.parse(src, None).expect("AST");

        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");

        assert_eq!(
            class.name(),
            "A".to_string(),
            "Equality of {} and {}",
            class.name(),
            "A".to_string()
        );
    }

    #[test]
    fn parse_annotated_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
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

        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");

        assert_eq!(class.name(), "DEMO_CLASS".to_string());
    }
    #[test]
    fn parse_procedure() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        let src = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";
        let tree = parser.parse(src, None).unwrap();
        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "f".to_string());
    }

    #[test]
    fn parse_attribute() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        let src = "
class A
feature
    x: INTEGER
end
";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "x".to_string());
    }
    #[test]
    fn parse_model_names() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
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

        let model_names =
            ModelNames::parse(&tree.root_node(), src).expect("fails to parse model names");

        assert!(!model_names.0.is_empty());
        assert_eq!(model_names.0.first(), Some(&"seq".to_string()));
    }
    #[test]
    fn parse_model() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
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

        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");
        let model = class.model().clone();
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(
            features.first().expect("Parsed first feature").name(),
            "x".to_string()
        );
        assert_eq!(
            (&model.0.first().expect("Parsed model")).name(),
            "seq".to_string()
        );
    }
    #[test]
    fn parse_ancestors() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        let src = "
class A
inherit {NONE}
  X Y Z

inherit
  W
    undefine a
    redefine c
    rename e as f
    export
      {ANY}
        -- Header comment
        all
    select g
    end
end
";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::parse(&tree.root_node(), src).expect("fails to parse class");
        let mut ancestors = class.ancestors().iter();

        assert_eq!(class.name(), "A".to_string());

        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse first ancestor")
                .name(),
            "X".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse second ancestor")
                .name(),
            "Y".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse third ancestor")
                .name(),
            "Z".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse forth ancestor")
                .name(),
            "W".to_string()
        );
    }
    #[test]
    fn class_to_workspacesymbol() -> Result<()> {
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
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let file = processed_file::ProcessedFile::new(&mut parser, path.clone())?;
        let class = (&file).class();
        let symbol = <lsp_types::WorkspaceSymbol>::try_from(class);
        assert!(symbol.is_ok());
        Ok(())
    }
}
