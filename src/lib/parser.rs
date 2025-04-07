use anyhow::Context;
use streaming_iterator::StreamingIterator;
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

struct Parser(TreeSitterParser);

impl Parser {
    fn new() -> Self {
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
}

struct ParsedSource<'source> {
    source: &'source [u8],
    tree: Tree,
}

impl<'source> ParsedSource<'source> {
    fn classes(&self) -> Vec<(Class, Location, Range)> {
        todo!()
    }
    fn features(&self) -> Vec<(Feature, Location, Range)> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Parser {
        pub fn mock_tree(&self) -> Tree {
            todo!()
        }
    }

    // #[test]
    // fn parse() -> anyhow::Result<()> {
    //     let mut parser = Parser::new();
    //     let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
    //     let class: Vec<(Class, Location, Range)> = parsed_source.classes();
    //     let features: Vec<(Feature, Location, Range)> = parsed_source.features();
    //     Ok(())
    // }
}
