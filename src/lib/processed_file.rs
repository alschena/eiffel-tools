use super::code_entities::Class;
use super::code_entities::Feature;
use super::code_entities::Range;
use super::tree_sitter::ExtractedFrom;
use anyhow::Context;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Tree, TreeCursor};

pub(crate) struct ProcessedFile {
    pub(super) tree: Tree,
    pub(super) path: PathBuf,
}
impl ProcessedFile {
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> ProcessedFile {
        let src: String = String::from_utf8(std::fs::read(&path).expect("Failed to read file."))
            .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        ProcessedFile { tree, path }
    }
    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }
    pub(crate) fn feature_around(&self, range: Range) -> Option<Feature> {
        self.class()
            .expect("Parse class")
            .into_features()
            .into_iter()
            .find(|x| range <= *x.range())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn class(&self) -> anyhow::Result<Class> {
        let src: String =
            String::from_utf8(std::fs::read(&self.path).context("Failed to read file.")?)
                .context("Source code must be UTF8 encoded")?;
        let mut class =
            Class::extract_from(&mut self.tree.walk(), src.as_str()).context("Parsing of class")?;
        class.add_location(&self.path);
        Ok(class)
    }
}
