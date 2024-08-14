use super::processed_file::ProcessedFile;
use std::path::PathBuf;
use tree_sitter::{Node, TreeCursor};

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
    path: PathBuf,
    class: Class<'a>,
    features: Vec<Feature<'a>>,
}

impl CodeEntities<'_> {
    pub(crate) fn from_processed_file(file: &ProcessedFile) -> CodeEntities<'_> {
        let mut cursor = file.tree.walk();
        let mut class: Option<Class> = None;
        let mut features: Vec<Feature> = Vec::new();

        let mut nodes = Vec::new();
        let mut next_child = true;
        while next_child {
            let mut next_sibling = true;

            while next_sibling {
                let node = cursor.node();
                nodes.push(node);

                let name = node.kind();
                match name {
                    "class_declaration" => {
                        cursor.goto_first_child();
                        let mut next_sibling = true;
                        while next_sibling {
                            let node = cursor.node();
                            let name = node.kind();
                            if name == "class_name" {
                                let name =
                                    match String::from_utf8(file.src[node.byte_range()].to_vec()) {
                                        Ok(v) => v.to_uppercase(),
                                        Err(e) => panic!("invalid UTF-8 sequence {}", e),
                                    };
                                class = Some(Class::from_name(name));
                            }
                            next_sibling = cursor.goto_next_sibling();
                        }
                        cursor.reset(node);
                    }
                    "extended_feature_name" => {
                        let name = match String::from_utf8(file.src[node.byte_range()].to_vec()) {
                            Ok(v) => v,
                            Err(e) => panic!("invalid UTF-8 sequence {}", e),
                        };
                        features.push(Feature::from_name(name));
                    }
                    _ => {}
                }
                next_sibling = cursor.goto_next_sibling();
            }
            cursor.reset(nodes.pop().expect("This level not empty"));

            next_child = cursor.goto_first_child();
            while (!next_child) && (!nodes.is_empty()) {
                cursor.reset(nodes.pop().unwrap());
                next_child = cursor.goto_first_child();
            }
        }
        let class = class.expect(format!("No class found in file {:?}", file.path).as_str());
        CodeEntities {
            path: file.path.clone(),
            class,
            features,
        }
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
        let basic_path: PathBuf = PathBuf::from(BASIC_CLASS_PATH);
        let mut file = File::create(&basic_path)?;
        file.write_all(BASIC_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, basic_path.clone());

        let CodeEntities {
            path: _,
            class,
            features: _,
        } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("A".to_string()));

        Ok(())
    }

    #[test]
    fn process_annotated_class() -> std::io::Result<()> {
        let annotated_class_path: PathBuf = PathBuf::from(ANNOTATED_CLASS_PATH);
        let mut file = File::create(&annotated_class_path)?;
        file.write_all(ANNOTATED_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, annotated_class_path.clone());

        let CodeEntities {
            path: _,
            class,
            features: _,
        } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("DEMO_CLASS".to_string()));

        Ok(())
    }

    #[test]
    fn process_procedure() -> std::io::Result<()> {
        let procedure_path: PathBuf = PathBuf::from(PROCEDURE_PATH);
        let mut file = File::create(&procedure_path)?;
        file.write_all(PROCEDURE.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, procedure_path.clone());

        let CodeEntities {
            path: _,
            class,
            features,
        } = CodeEntities::from_processed_file(&file);

        assert_eq!(class, Class::from_name("A".to_string()));
        assert_eq!(features, vec![Feature::from_name("f".to_string())]);

        Ok(())
    }
}
