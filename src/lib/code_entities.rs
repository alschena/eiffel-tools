use super::tree_sitter::WidthFirstTraversal;
use tree_sitter::Tree;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum FeatureVisibility<'a> {
    Private,
    Some(&'a Class<'a>),
    Public,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Feature<'a> {
    name: String,
    visibility: FeatureVisibility<'a>,
}
impl Feature<'_> {
    fn from_name<'a>(name: String) -> Feature<'a> {
        let visibility = FeatureVisibility::Private;
        Feature { name, visibility }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Class<'a> {
    name: String,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
}

impl Class<'_> {
    pub(crate) fn from_name<'a>(name: String) -> Class<'a> {
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
}

pub(crate) struct CodeEntities<'a> {
    class: Class<'a>,
    features: Vec<Feature<'a>>,
}

impl CodeEntities<'_> {
    pub(crate) fn from_tree_and_src<'a, 'b>(tree: &'a Tree, src: &'a Vec<u8>) -> CodeEntities<'b> {
        let cursor = tree.walk();
        let mut traversal = WidthFirstTraversal::new(cursor);

        let class = match String::from_utf8(
            src[traversal
                .find(|x| x.kind() == "class_name")
                .expect("class_name")
                .byte_range()]
            .to_vec(),
        ) {
            Ok(name) => Class::from_name(name.to_uppercase()),
            Err(e) => panic!("invalid UTF-8 sequence {}", e),
        };

        let features = traversal
            .filter(|x| x.kind() == "extended_feature_name")
            .map(
                |node| match String::from_utf8(src[node.byte_range()].to_vec()) {
                    Ok(v) => Feature::from_name(v),
                    Err(e) => panic!("invalid UTF-8 sequence {}", e),
                },
            )
            .collect();

        CodeEntities { class, features }
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

        let CodeEntities { class, features: _ } =
            CodeEntities::from_tree_and_src(&tree, &src.into());

        assert_eq!(class, Class::from_name("A".to_string()));

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

        let CodeEntities { class, features: _ } =
            CodeEntities::from_tree_and_src(&tree, &src.into());

        assert_eq!(class, Class::from_name("DEMO_CLASS".to_string()));

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
        let CodeEntities { class, features } = CodeEntities::from_tree_and_src(&tree, &src.into());

        assert_eq!(class, Class::from_name("A".to_string()));
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

        let CodeEntities { class, features } = CodeEntities::from_tree_and_src(&tree, &src.into());

        assert_eq!(class, Class::from_name("A".to_string()));
        assert_eq!(features, vec![Feature::from_name("x".to_string())]);

        Ok(())
    }
}
