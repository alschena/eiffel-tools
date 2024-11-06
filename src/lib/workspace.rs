use crate::lib::code_entities::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

pub struct Workspace {
    ecf_path: Option<PathBuf>,
    files: Vec<ProcessedFile>,
}

impl Workspace {
    pub(crate) fn new() -> Workspace {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        Workspace { files: Vec::new(), ecf_path: None }
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
    pub fn find_file(&self, path: &Path) -> Option<&ProcessedFile> {
        self.files.iter().find(|&x| x.path == path)
    }
    pub(crate) fn set_ecf_path(&mut self, ecf_path: Option<PathBuf>) {
        self.ecf_path = ecf_path
    }
    pub(crate) fn ecf_path(&self) -> Option<PathBuf> {
        self.ecf_path.clone()
    }
}
