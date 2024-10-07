use crate::lib::processed_file::ProcessedFile;
use std::path::Path;
use tree_sitter::Parser;

pub struct Workspace {
    files: Vec<ProcessedFile>,
    parser: Parser,
}

impl Workspace {
    pub(crate) fn new() -> Workspace {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        Workspace {
            files: Vec::new(),
            parser,
        }
    }
    pub(crate) fn add_file(&mut self, filepath: &Path) {
        self.files
            .push(ProcessedFile::new(&mut self.parser, filepath.to_owned()))
    }
    pub(crate) fn add_processed_file(&mut self, file: ProcessedFile) {
        self.files.push(file)
    }
    pub(crate) fn init_classes(&mut self) {}
    pub(crate) fn files(&self) -> &Vec<ProcessedFile> {
        &self.files
    }
}
