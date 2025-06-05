use anyhow::Context;
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tracing::instrument;

use ::tree_sitter::Node;
use ::tree_sitter::Parser as TreeSitterParser;
use ::tree_sitter::Query;
use ::tree_sitter::QueryCursor;
pub use ::tree_sitter::Tree;

use super::code_entities::prelude::*;

mod class_tree;
use class_tree::FeatureTree;

mod expression_tree;
pub use expression_tree::ExpressionTree;

mod util;
pub use util::TreeTraversal;

pub struct Parser(TreeSitterParser);

impl Parser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        Self(parser)
    }

    #[instrument(skip_all)]
    pub fn parse<'source, T>(&mut self, source: &'source T) -> Result<ParsedSource<'source>>
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

    #[cfg(test)]
    pub fn class_from_source<'source, T>(&mut self, source: &'source T) -> Result<Class>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        self.class_and_tree_from_source(source)
            .map(|(class, _)| class)
    }

    #[instrument(skip_all)]
    pub fn class_and_tree_from_source<S>(&mut self, source: S) -> Result<(Class, Tree)>
    where
        S: AsRef<[u8]>,
    {
        let parsed_source = self.parse(source.as_ref())?;
        let mut traversal = parsed_source.class_tree_traversal()?;
        traversal.class().map(|class| (class, parsed_source.tree))
    }

    pub fn feature_from_source<T>(&mut self, source: &T) -> Result<Feature>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let parsed_source = self.parse(source)?;
        let mut feature_tree_traversal = parsed_source.feature_tree_traversal()?;
        let mut alias_features = feature_tree_traversal.feature()?;
        let any_feature = alias_features.pop().with_context(
            || "fails to get a feature from a vector of alias features parsing source: {source}",
        )?;
        Ok(any_feature)
    }
}

pub struct ParsedSource<'source> {
    source: &'source [u8],
    pub tree: Tree,
}

impl ParsedSource<'_> {
    fn class_tree_traversal(&self) -> Result<TreeTraversal<'_, '_>> {
        TreeTraversal::try_new(self.source, self.tree.root_node(), class_tree::query())
    }

    fn feature_tree_traversal(&self) -> Result<TreeTraversal<'_, '_>> {
        TreeTraversal::try_new(
            self.source,
            self.tree.root_node(),
            <TreeTraversal as FeatureTree>::query(),
        )
    }

    pub fn expression_tree_traversal(&self) -> Result<TreeTraversal<'_, '_>> {
        TreeTraversal::try_new(
            self.source,
            self.tree.root_node(),
            <TreeTraversal as ExpressionTree>::query_top_level_identifiers(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub const EMPTY_CLASS: &str = r#"class A end"#;

    #[tokio::test]
    async fn process_file() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let (class, _tree) = parser.class_and_tree_from_source(EMPTY_CLASS)?;

        assert_eq!(class.name(), "A", "class name: {:#?}", class.name());
        Ok(())
    }
}
