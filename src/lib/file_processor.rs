use super::code_entities::{Class, Feature};
use std::path::PathBuf;
use tree_sitter::{Parser, Tree, TreeCursor};

pub(crate) struct FileProcessor {
    tree: Tree,
    path: PathBuf,
    src: Vec<u8>,
}
impl FileProcessor {
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> FileProcessor {
        let src = std::fs::read(&path).expect("Failed to read file.");
        let tree = parser.parse(&src, None).unwrap();
        FileProcessor { tree, path, src }
    }
    pub(crate) fn class(&mut self) -> Option<Class> {
        let mut cursor = self.tree.walk();
        self.class_helper(&mut cursor)
    }
    fn class_helper<'a>(&self, c: &mut TreeCursor) -> Option<Class<'a>> {
        if c.node().kind() == "class_name" {
            let name = match String::from_utf8(self.src[c.node().byte_range()].to_vec()) {
                Ok(v) => v.to_uppercase(),
                Err(e) => panic!("invalid UTF-8 sequence {}", e),
            };
            let source_file = self.path.clone();
            Some(Class::from_name_and_path(name, source_file))
        } else {
            if c.goto_next_sibling() {
                self.class_helper(c)
            } else {
                if c.goto_first_child() {
                    self.class_helper(c)
                } else {
                    None
                }
            }
        }
    }
    pub(crate) fn process<'a>(&mut self) -> ProcessedFile<'a> {
        let mut cursor = self.tree.walk();
        let mut next_child = true;
        let mut class: Option<Class<'a>> = None;
        let mut features: Vec<Feature<'a>> = Vec::new();

        while next_child {
            let mut next_sibling = true;

            while next_sibling {
                let node = cursor.node();
                let name = node.kind();

                match name {
                    "class_name" => {
                        let name = match String::from_utf8(self.src[node.byte_range()].to_vec()) {
                            Ok(v) => v.to_uppercase(),
                            Err(e) => panic!("invalid UTF-8 sequence {}", e),
                        };
                        let source_file = self.path.clone();
                        class = Some(Class::from_name_and_path(name, source_file))
                    }
                    "extended_feature_name" => {
                        let name = match String::from_utf8(self.src[node.byte_range()].to_vec()) {
                            Ok(v) => v,
                            Err(e) => panic!("invalid UTF-8 sequence {}", e),
                        };
                        let source_file = self.path.clone();
                        features.push(todo!());
                    }
                    _ => next_sibling = cursor.goto_next_sibling(),
                }
            }
            next_child = cursor.goto_first_child()
        }
        let class = class.expect(format!("No class found in file {:?}", self.path).as_str());
        ProcessedFile {
            path: self.path.clone(),
            class,
            features,
        }
    }
}

pub(crate) struct ProcessedFile<'a> {
    path: PathBuf,
    class: Class<'a>,
    features: Vec<Feature<'a>>,
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

    use std::fs::File;
    use std::io::prelude::*;

    #[test]
    fn basic_class() -> std::io::Result<()> {
        let basic_path: PathBuf = PathBuf::from(BASIC_CLASS_PATH);
        let mut file = File::create(&basic_path)?;
        file.write_all(BASIC_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let mut processor = FileProcessor::new(&mut parser, basic_path.clone());

        let class = match processor.class() {
            Some(c) => c,
            None => panic!("no_class"),
        };

        assert_eq!(
            class,
            Class::from_name_and_path("A".to_string(), basic_path)
        );

        Ok(())
    }

    #[test]
    fn annotated_class() -> std::io::Result<()> {
        let basic_path: PathBuf = PathBuf::from(ANNOTATED_CLASS_PATH);
        let mut file = File::create(&basic_path)?;
        file.write_all(ANNOTATED_CLASS.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let mut processor = FileProcessor::new(&mut parser, basic_path.clone());

        let class = match processor.class() {
            Some(c) => c,
            None => panic!("no_class"),
        };

        assert_eq!(
            class,
            Class::from_name_and_path("DEMO_CLASS".to_string(), basic_path)
        );

        Ok(())
    }
}
