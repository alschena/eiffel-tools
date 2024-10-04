use super::code_entities::Class;
use super::code_entities::Feature;
use super::code_entities::Range;
use super::tree_sitter::Extract;
use std::path::PathBuf;
use tree_sitter::{Parser, Tree};

pub(crate) struct ProcessedFile {
    pub(super) tree: Tree,
    pub(super) path: PathBuf,
    pub(super) src: String,
}
impl ProcessedFile {
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> ProcessedFile {
        let src: String = String::from_utf8(std::fs::read(&path).expect("Failed to read file."))
            .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        ProcessedFile { tree, path, src }
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
    pub(crate) fn class(&self) -> anyhow::Result<Class> {
        let mut class = Class::extract(&mut self.tree.walk(), self.src.as_str())?;
        class.add_location(&self.path);
        Ok(class)
    }
}
