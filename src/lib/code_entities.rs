use super::{processed_file::ProcessedFile, tree_sitter::WidthFirstTraversal};
use tree_sitter::Tree;

pub(super) struct Point {
    pub(super) row: usize,
    pub(super) column: usize,
}

pub(super) struct Range {
    pub(super) start: Point,
    pub(super) end: Point,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum FeatureVisibility<'a> {
    Private,
    Some(&'a Class<'a>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Feature<'a> {
    name: String,
    visibility: FeatureVisibility<'a>,
}
impl Feature<'_> {
    pub(super) fn from_name<'a>(name: String) -> Feature<'a> {
        let visibility = FeatureVisibility::Private;
        Feature { name, visibility }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Class<'a> {
    name: String,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
}

impl<'c> Class<'c> {
    pub(crate) fn from_name(name: String) -> Class<'c> {
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            features,
            descendants,
            ancestors,
        }
    }
    pub(crate) fn from_name_and_features(name: String, features: Vec<Feature<'c>>) -> Class<'c> {
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            features,
            descendants,
            ancestors,
        }
    }

    fn from_tree_and_src<'a>(tree: &'a Tree, src: &'a str) -> Class<'c> {
        let cursor = tree.walk();
        let mut traversal = WidthFirstTraversal::new(cursor);

        let name = src[traversal
            .find(|x| x.kind() == "class_name")
            .expect("class_name")
            .byte_range()]
        .into();

        let features = traversal
            .filter(|x| x.kind() == "extended_feature_name")
            .map(|node| Feature::from_name(src[node.byte_range()].into()))
            .collect();

        Class {
            name,
            features,
            descendants: Vec::new(),
            ancestors: Vec::new(),
        }
    }
    pub(crate) fn add_feature(&mut self, feature: Feature<'c>) {
        self.features.push(feature)
    }
}

impl From<&ProcessedFile> for Class<'_> {
    fn from(value: &ProcessedFile) -> Self {
        Class::from_tree_and_src(&value.tree, &value.src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_base_class() -> std::io::Result<()> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
class A
note
end
    ";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::from_tree_and_src(&tree, &src);

        assert_eq!(class.name, "A".to_string());

        Ok(())
    }

    #[test]
    fn process_annotated_class() -> std::io::Result<()> {
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
        let tree = parser.parse(src, None).unwrap();

        let class = Class::from_tree_and_src(&tree, &src);

        assert_eq!(class.name, "DEMO_CLASS".to_string());

        Ok(())
    }

    #[test]
    fn process_procedure() -> std::io::Result<()> {
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
        let class = Class::from_tree_and_src(&tree, &src);
        let features = class.features.clone();

        assert_eq!(class.name, "A".to_string());
        assert_eq!(features, vec![Feature::from_name("f".to_string())]);

        Ok(())
    }

    #[test]
    fn process_attribute() -> std::io::Result<()> {
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

        let class = Class::from_tree_and_src(&tree, &src);
        let features = class.features.clone();

        assert_eq!(class.name, "A".to_string());
        assert_eq!(features, vec![Feature::from_name("x".to_string())]);

        Ok(())
    }
}
