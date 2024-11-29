use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use std::path::Path;

pub struct Workspace {
    files: Vec<ProcessedFile>,
}

impl Workspace {
    pub(crate) fn new() -> Workspace {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        Workspace { files: Vec::new() }
    }
    pub(crate) fn set_files(&mut self, files: Vec<ProcessedFile>) {
        self.files = files
    }
    pub(crate) fn files(&self) -> &Vec<ProcessedFile> {
        &self.files
    }
    pub fn find_file(&self, path: &Path) -> Option<&ProcessedFile> {
        self.files.iter().find(|&x| x.path == path)
    }
    pub fn system_classes(&self) -> impl Iterator<Item = &Class> {
        self.files().into_iter().map(|file| file.class())
    }
}
