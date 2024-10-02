use super::code_entities::class::Class;
use crate::lib::processed_file::ProcessedFile;
use std::collections::HashMap;
use std::path::PathBuf;
use tree_sitter::{Parser, Tree, TreeCursor};

struct Workspace {
    classes: HashMap<PathBuf, Vec<Class>>,
    files: HashMap<PathBuf, ProcessedFile>,
    parser: Parser,
}

impl Workspace {
    fn new() -> Workspace {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        Workspace {
            classes: HashMap::new(),
            files: HashMap::new(),
            parser,
        }
    }
    fn add_file(&mut self, filepath: PathBuf) {}
    fn init_classes(&mut self) {}
}
