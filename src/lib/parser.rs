use crate::lib::processed_file::ProcessedFile;
use anyhow::Context;
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use util::TreeTraversal;
mod tree_sitter_extension;
pub use tree_sitter_extension::*;

use ::tree_sitter::Node;
use ::tree_sitter::Parser as TreeSitterParser;
use ::tree_sitter::Query;
use ::tree_sitter::QueryCursor;
use ::tree_sitter::QueryMatches;
use ::tree_sitter::Tree;

use super::code_entities::prelude::*;

mod class_tree;
use class_tree::ClassTree;

mod util;

pub struct Parser(TreeSitterParser);

impl Parser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        Self(parser)
    }

    fn parse<'source, T>(&mut self, source: &'source T) -> anyhow::Result<ParsedSource<'source>>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let source = source.as_ref();
        let tree = self
            .0
            .parse(source, None)
            .with_context(|| "fails to parse source: {source:?}")?;
        Ok(ParsedSource { source, tree })
    }

    #[instrument(skip(self))]
    pub async fn process_file(&mut self, path: PathBuf) -> anyhow::Result<ProcessedFile> {
        let src = String::from_utf8(
            tokio::fs::read(&path)
                .await
                .with_context(|| format!("fails to read file at path: {path:#?}"))?,
        )?;
        eprintln!("path: {path:#?}");
        let parsed_source = self
            .parse(&src)
            .with_context(|| "fails processing file at path: {path:#?}")?;
        let mut class_tree = TreeTraversal::try_from(&parsed_source)
            .with_context(|| "fails processing file at path: {path:#?}")?;
        let class = class_tree
            .class()
            .with_context(|| "fails processing file at path: {path:#?}")?;
        Ok(ProcessedFile {
            tree: parsed_source.tree,
            path,
            class,
        })
    }
}

struct ParsedSource<'source> {
    source: &'source [u8],
    pub tree: Tree,
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    pub const EMPTY_CLASS: &str = r#"class A end"#;

    #[tokio::test]
    async fn process_file() -> anyhow::Result<()> {
        let mut parser = Parser::new();

        let tmp_dir = TempDir::new().expect("fails to create temporary directory.");
        let tmp_file = tmp_dir.child("tmp_file.e");
        tmp_file.write_str(EMPTY_CLASS)?;
        assert!(tmp_file.exists(), "tmp file exists");

        let processed_file = parser.process_file(tmp_file.to_path_buf()).await?;

        assert_eq!(processed_file.class.name, ClassName("A".to_string()));
        Ok(())
    }
}
