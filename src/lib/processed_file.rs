use super::code_entities::prelude::*;
use super::tree_sitter_extension::{Parse, WidthFirstTraversal};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::instrument;
use tree_sitter::{Parser, Tree};

/// Stores all the information of a file
pub(crate) struct ProcessedFile {
    /// Treesitter abstract syntax tree, stored for incremental parsing.
    pub(super) tree: Tree,
    /// Path of the file
    pub(super) path: PathBuf,
    /// In eiffel a class contains all other code entities of a class
    pub(super) class: Class,
}
impl ProcessedFile {
    #[instrument(skip(parser))]
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> Result<ProcessedFile> {
        let src: String = String::from_utf8(std::fs::read(&path).expect("Failed to read file."))
            .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        let mut class =
            Class::parse(&tree.root_node(), src.as_str()).context("Parsing of class")?;
        class.add_location(&path);
        Ok(ProcessedFile { tree, path, class })
    }
    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }
    pub(crate) fn feature_around(&self, range: Range) -> Option<Feature> {
        let mut features = self.class().features().iter();
        match features.find(|x| range <= *x.range()) {
            Some(f) => Some(f.clone()),
            None => None,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn class(&self) -> &Class {
        &self.class
    }
    pub(crate) fn code_entity_at_point_in_src(
        &self,
        point: Point,
        src: &[u8],
    ) -> Result<Box<dyn CodeEntity>> {
        unimplemented!();
    }
}
