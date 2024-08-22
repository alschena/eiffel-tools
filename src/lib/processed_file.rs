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
}
