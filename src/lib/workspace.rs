use crate::lib::code_entities::prelude::*;
use crate::lib::config::System;
use crate::lib::processed_file::ProcessedFile;
use std::path::Path;
use tokio::task::JoinSet;

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
        self.files.iter().find(|&x| x.path() == path)
    }
    pub fn find_file_mut(&mut self, path: &Path) -> Option<&mut ProcessedFile> {
        self.files.iter_mut().find(|x| x.path() == path)
    }
    pub fn system_classes(&self) -> Vec<Class> {
        self.files()
            .into_iter()
            .map(|file| file.class())
            .cloned()
            .collect()
    }
    pub async fn load_system(&mut self, system: &System) {
        let eiffel_files = system.eiffel_files();
        let mut set = JoinSet::new();
        eiffel_files.into_iter().for_each(|filepath| {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_eiffel::LANGUAGE.into())
                .expect("load eiffel grammar.");
            set.spawn(async move { ProcessedFile::new(&mut parser, filepath.to_owned()).await });
        });
        let files = (set.join_all().await)
            .into_iter()
            .filter_map(|file| file)
            .collect();
        self.set_files(files);
    }
}
