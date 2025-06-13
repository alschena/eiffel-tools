use anyhow::Context;
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tracing::instrument;

use ::tree_sitter::Node;
use ::tree_sitter::Parser as TreeSitterParser;
use ::tree_sitter::Query;
use ::tree_sitter::QueryCursor;
pub use ::tree_sitter::Tree;

use super::code_entities::contract::*;
use super::code_entities::prelude::*;

mod class_tree;
use class_tree::FeatureTree;

mod expression_tree;
pub use expression_tree::ExpressionTree;

mod util;
pub use util::TreeTraversal;

pub struct Parser(TreeSitterParser);

#[derive(Clone)]
pub enum Parsed<T> {
    Correct(T),
    HasErrorNodes(Tree, Vec<u8>),
}

impl<T: std::fmt::Debug> std::fmt::Debug for Parsed<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parsed::Correct(val) => write!(f, "Parsed correctly: {:#?}", val),
            Parsed::HasErrorNodes(tree, items) => write!(
                f,
                "Parses code:\n{:#?}\n\nTo the AST:\n{:#?}",
                String::from_utf8(items.to_owned()),
                tree.root_node().to_sexp()
            ),
        }
    }
}

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

    pub fn to_feature<T>(&mut self, source: &T) -> Result<Parsed<Feature>>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let prepended_source = {
            let mut prefix: Vec<u8> = "[FEATURE]\n".into();
            prefix.extend_from_slice(source.as_ref());
            prefix
        };

        let parsed_source = self.parse(&prepended_source)?;

        if parsed_source.tree.root_node().has_error() {
            Ok(Parsed::HasErrorNodes(
                parsed_source.tree,
                source.as_ref().to_owned(),
            ))
        } else {
            let mut feature_tree_traversal = parsed_source.feature_tree_traversal()?;
            let mut alias_features = feature_tree_traversal.feature()?;
            let any_feature = alias_features.pop().with_context(
                || "fails to get a feature from a vector of alias features parsing source: {source}",
            )?;
            Ok(Parsed::Correct(any_feature))
        }
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
    use crate::code_entities::contract::{Clause, Postcondition};

    use super::*;
    pub const EMPTY_CLASS: &str = r#"class A end"#;

    #[tokio::test]
    async fn process_file() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let (class, _tree) = parser.class_and_tree_from_source(EMPTY_CLASS)?;

        assert_eq!(class.name(), "A", "class name: {:#?}", class.name());
        Ok(())
    }

    #[test]
    fn predicate_identifiers() {
        let mut parser = Parser::new();
        let parsed_source = parser
            .parse("[EXPRESSION]\nx < y.z.w")
            .expect("fails to parse expression");
        let mut tree = parsed_source
            .expression_tree_traversal()
            .expect("fails to create expression tree traversal.");

        let ids = tree
            .top_level_identifiers()
            .expect("fails to get top level identifiers");

        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.len() == 2);
    }

    #[test]
    fn predicate_identifiers_unqualified_calls() {
        let mut parser = Parser::new();
        let parsed_source = parser
            .parse("[EXPRESSION]\nx (y) < y (l).z.w")
            .expect("fails to parse expression");
        let mut tree = parsed_source
            .expression_tree_traversal()
            .expect("fails to create expression tree traversal.");

        let ids = tree
            .top_level_identifiers()
            .expect("fails to get top level identifiers");

        eprintln!("{ids:?}");
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.contains("l"));
        assert!(ids.len() == 3);
    }

    #[test]
    fn parse_feature() {
        let mut parser = Parser::new();
        let parsed_feature = parser
            .to_feature(
                r#"absolute_short (num: INTEGER_16): INTEGER_16
		do
			if 0 > num then
				Result := -num
			else
				Result := num
			end
		ensure
			same_when_non_negative: 0 <= num implies Result = num
			other_sign_when_negative: num < 0 implies Result = - num
		end
"#,
            )
            .expect("fails to parse feature");

        match parsed_feature {
            Parsed::Correct(val) => {
                assert_eq!(val.name(), "absolute_short");
            }
            Parsed::HasErrorNodes(ref tree, ref _items) => {
                assert!(!tree.root_node().has_error(), "{:#?}", &parsed_feature);
            }
        }
    }
}
