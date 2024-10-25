use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use anyhow::Result;
use std::path::Path;
use tree_sitter::Parser;

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
    pub(crate) fn classes(&self) -> Vec<&Class> {
        self.files().iter().map(|f| f.class()).collect()
    }
}
