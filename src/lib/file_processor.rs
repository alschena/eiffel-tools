use super::code_entities::{Class, Feature};
use std::path::PathBuf;
use tree_sitter::{Parser, Tree, TreeCursor};

struct FileProcessor {
    tree: Tree,
    path: PathBuf,
    src: Vec<u8>,
}
impl FileProcessor {
    fn new(parser: &mut Parser, path: PathBuf) -> FileProcessor {
        let src = std::fs::read(&path).expect("Failed to read file.");
        let tree = parser.parse(&src, None).unwrap();
        FileProcessor { tree, path, src }
    }
    fn class(&mut self) -> Option<Class> {
        let mut cursor = self.tree.walk();
        self.class_helper(&mut cursor)
    }
    fn class_helper<'a>(&self, c: &mut TreeCursor) -> Option<Class<'a>> {
        if c.node().kind() == "class_name" {
            let name = match String::from_utf8(self.src[c.node().byte_range()].to_vec()) {
                Ok(v) => v,
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
    fn features(&mut self) -> Vec<Feature> {
        todo!()
    }
}
