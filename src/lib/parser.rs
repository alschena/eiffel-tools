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

struct Parser {}
impl Parser {
    fn new() -> Self {
        todo!()
    }
    fn tree<'source, T>(&mut self, source: &'source T) -> Tree
    where
        T: AsRef<[u8]> + ?Sized,
    {
        todo!()
    }
    fn parse<'source, T>(&mut self, source: &'source T) -> ParsedSource
    where
        T: AsRef<[u8]> + ?Sized,
    {
        todo!()
    }
    fn class_tree<'s, 'source, 'tree, T: ClassTree>(&'s self) -> T
    where
        's: 'source,
        's: 'tree,
    {
        todo!()
    }
}

struct ParsedSource {}
impl ParsedSource {
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

    pub const DOUBLE_FEATURE_CLASS_SOURCE: &str = r#"
        class
            TEST
        feature
            x: INTEGER
            y: INTEGER
        end
    "#;

    pub const ANNOTATED_CLASS_SOURCE: &str = r#"
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    "#;

    pub const MODEL_CLASS_SOURCE: &str = r#"
note
    model: seq
class A
feature
    x: INTEGER
    seq: MML_SEQUENCE [INTEGER]
end
"#;

    impl Parser {
        pub fn mock_tree(&self) -> Tree {
            todo!()
        }
    }

    #[test]
    fn parse() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(MODEL_CLASS_SOURCE);
        let class: Vec<(Class, Location, Range)> = parsed_source.classes();
        let features: Vec<(Feature, Location, Range)> = parsed_source.features();
        Ok(())
    }
}
