use super::code_entities::{Class, Feature, Range};
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
    pub(crate) fn feature_around(&self, range: Range) -> Option<Box<Feature>> {
        Class::try_from(self)
            .expect("Parse class")
            .into_features()
            .into_iter()
            .find(|x| range <= *x.range())
    }
}

impl TryFrom<&ProcessedFile> for Class {
    type Error = anyhow::Error;

    fn try_from(value: &ProcessedFile) -> Result<Self, Self::Error> {
        let mut class = Class::try_from((&value.tree, value.src.as_str()))?;
        class.add_location(&value.path);
        Ok(class)
    }
}
