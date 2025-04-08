use crate::lib::processed_file::ProcessedFile;
use anyhow::Context;
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use util::TreeTraversal;

use ::tree_sitter::Node;
use ::tree_sitter::Parser as TreeSitterParser;
use ::tree_sitter::Query;
use ::tree_sitter::QueryCursor;
use ::tree_sitter::QueryMatches;
use ::tree_sitter::Tree;

use super::code_entities::prelude::*;

mod class_tree;
use class_tree::ClassTree;
use class_tree::FeatureTree;

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

    pub fn class_from_source<'source, T>(&mut self, source: &'source T) -> anyhow::Result<Class>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        self.class_and_tree_from_source(source)
            .map(|(_, class)| class)
    }

    pub fn class_and_tree_from_source<'source, T>(
        &mut self,
        source: &'source T,
    ) -> anyhow::Result<(Tree, Class)>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let parsed_source = self.parse(source)?;
        let mut traversal: TreeTraversal<'source, '_> = (&parsed_source).try_into()?;
        traversal.class().map(|class| (parsed_source.tree, class))
    }

    pub fn feature_from_source<'source, T>(&mut self, source: &'source T) -> anyhow::Result<Feature>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let parsed_source = self.parse(source)?;
        let mut feature_tree_traversal = parsed_source.feature_tree_traversal()?;
        let mut alias_features = feature_tree_traversal.feature()?;
        let any_feature = alias_features.pop().with_context(|| {
            "fails to get a feature from a vector of alias features parsing source: {source}"
        })?;
        Ok(any_feature)
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

impl ParsedSource<'_> {
    fn class_tree_traversal(&self) -> anyhow::Result<TreeTraversal<'_, '_>> {
        TreeTraversal::try_new(
            self.source,
            self.tree.root_node(),
            <TreeTraversal as ClassTree>::query(),
        )
    }

    fn feature_tree_traversal(&self) -> anyhow::Result<TreeTraversal<'_, '_>> {
        TreeTraversal::try_new(
            self.source,
            self.tree.root_node(),
            <TreeTraversal as FeatureTree>::query(),
        )
    }

    fn class(&self) -> anyhow::Result<Class> {
        let mut traversal = TreeTraversal::try_from(self)?;
        traversal.class()
    }
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
