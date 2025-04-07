use crate::lib::code_entities::prelude::*;
use crate::lib::config::System;
use crate::lib::parser::Parser;
use crate::lib::processed_file::ProcessedFile;
use std::path::Path;
use tokio::task::JoinSet;
use tracing::warn;

#[derive(Debug)]
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
            set.spawn(async move {
                let mut parser = Parser::new();
                parser.process_file(filepath).await
            });
        });
        let files = (set.join_all().await)
            .into_iter()
            .filter_map(|file| match file {
                Ok(file) => Some(file),
                Err(e) => {
                    warn!("fails to parse file with error: {e}");
                    None
                }
            })
            .collect();
        self.set_files(files);
    }
}

#[cfg(test)]
pub mod tests {
    pub use super::*;

    impl Workspace {
        pub fn mock() -> Self {
            Self { files: Vec::new() }
        }
        pub fn is_mock(&self) -> bool {
            self.files.is_empty()
        }
    }
}
