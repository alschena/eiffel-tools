use super::code_entities::Class;
use std::collections::HashMap;
use std::path::PathBuf;
use tree_sitter::{Parser, Tree, TreeCursor};

struct Workspace<'a> {
    classes: Vec<Class<'a>>,
    files: HashMap<PathBuf, Tree>,
    parser: Parser,
}

impl Workspace<'_> {
    fn new() -> Workspace<'static> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        Workspace {
            classes: Vec::new(),
            files: HashMap::new(),
            parser,
        }
    }
    fn add_file(&mut self, filepath: PathBuf) {}
    fn init_classes(&mut self) {}
}
