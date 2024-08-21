use std::path::PathBuf;
use tree_sitter::{Parser, Tree, TreeCursor};

pub(crate) struct ProcessedFile {
    pub(super) tree: Tree,
    pub(super) path: PathBuf,
    pub(super) src: Vec<u8>,
}
impl ProcessedFile {
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> ProcessedFile {
        let src = std::fs::read(&path).expect("Failed to read file.");
        let tree = parser.parse(&src, None).unwrap();
        ProcessedFile { tree, path, src }
    }
}
