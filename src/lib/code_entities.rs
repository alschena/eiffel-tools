use super::processed_file::ProcessedFile;
use super::tree_sitter::WidthFirstTraversal;
use std::path::PathBuf;

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
    pub(crate) fn from_processed_file(file: &ProcessedFile) -> CodeEntities<'_> {
        let cursor = file.tree.walk();
        let mut traversal = WidthFirstTraversal::new(cursor);

        let class = match String::from_utf8(
            file.src[traversal
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
                |node| match String::from_utf8(file.src[node.byte_range()].to_vec()) {
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

    const BASIC_CLASS_PATH: &str = "/tmp/basic_class.e";
    const BASIC_CLASS: &str = "
class A
note
end
    ";

    const ANNOTATED_CLASS_PATH: &str = "/tmp/annotated_class.e";
    const ANNOTATED_CLASS: &str = "
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    ";

    const ATTRIBUTE_PATH: &str = "/tmp/class_with_feature_path.e";
    const ATTRIBUTE: &str = "
class A
feature
  x: INTEGER
end
";

    const PROCEDURE_PATH: &str = "/tmp/class_with_feature_path.e";
    const PROCEDURE: &str = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";

    use std::fs::File;
    use std::io::prelude::*;

    #[test]
    fn process_base_class() -> std::io::Result<()> {
        let path: PathBuf = PathBuf::from(BASIC_CLASS_PATH);
        let mut file = File::create(&path)?;
        file.write_all(BASIC_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, path.clone());

        let CodeEntities { class, features: _ } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("A".to_string()));

        Ok(())
    }

    #[test]
    fn process_annotated_class() -> std::io::Result<()> {
        let path: PathBuf = PathBuf::from(ANNOTATED_CLASS_PATH);
        let mut file = File::create(&path)?;
        file.write_all(ANNOTATED_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, path.clone());

        let CodeEntities { class, features: _ } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("DEMO_CLASS".to_string()));

        Ok(())
    }

    #[test]
    fn process_procedure() -> std::io::Result<()> {
        let path: PathBuf = PathBuf::from(PROCEDURE_PATH);
        let mut file = File::create(&path)?;
        file.write_all(PROCEDURE.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, path.clone());

        let CodeEntities { class, features } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("A".to_string()));
        assert_eq!(features, vec![Feature::from_name("f".to_string())]);

        Ok(())
    }

    #[test]
    fn process_attribute() -> std::io::Result<()> {
        let path: PathBuf = PathBuf::from(ATTRIBUTE_PATH);
        let mut file = File::create(&path)?;
        file.write_all(ATTRIBUTE.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, path.clone());

        let CodeEntities { class, features } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("A".to_string()));
        assert_eq!(features, vec![Feature::from_name("x".to_string())]);

        Ok(())
    }
}
