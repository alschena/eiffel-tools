use crate::lib::code_entities::{Class, Feature, Point, Range};
use std::ops::{Deref, DerefMut};

use tree_sitter::{Node, Tree, TreeCursor};

pub(crate) struct WidthFirstTraversal<'a> {
    cursor: TreeCursor<'a>,
    stack: Vec<Node<'a>>,
}

impl WidthFirstTraversal<'_> {
    pub(crate) fn new(cursor: TreeCursor<'_>) -> WidthFirstTraversal<'_> {
        let stack = Vec::new();
        WidthFirstTraversal { cursor, stack }
    }
}

impl<'a> Iterator for WidthFirstTraversal<'a> {
    type Item = Node<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            let node = self.cursor.node();
            self.stack.push(node);
            return Some(node);
        }
        if self.cursor.goto_next_sibling() {
            let node = self.cursor.node();
            self.stack.push(node);
            return Some(node);
        } else {
            loop {
                let cursor = &mut self.cursor;
                cursor.reset(self.stack.pop().expect("One node in the stack here"));
                if cursor.goto_first_child() {
                    let node = self.cursor.node();
                    self.stack.push(node);
                    return Some(node);
                }
                if self.stack.is_empty() {
                    return None;
                }
            }
        }
    }
}

impl From<tree_sitter::Point> for Point {
    fn from(value: tree_sitter::Point) -> Self {
        Self {
            row: value.row,
            column: value.column,
        }
    }
}

impl From<tree_sitter::Range> for Range {
    fn from(value: tree_sitter::Range) -> Self {
        Self {
            start: value.start_point.into(),
            end: value.end_point.into(),
        }
    }
}

impl<'c> Class<'c> {
    /// This relies on the first `class_name` node (containing the class associated to the current file) coming earliar than any "extended_feature_name" node in the `WidthFirstTraversal` of the tree-sitter tree.
    pub(super) fn from_tree_and_src<'a>(tree: &'a Tree, src: &'a str) -> Class<'c> {
        let cursor = tree.walk();
        let mut traversal = WidthFirstTraversal::new(cursor);

        let node = traversal
            .find(|x| x.kind() == "class_name")
            .expect("class_name");

        let name = src[node.byte_range()].into();
        let range = node.range().into();
        let mut class = Self::from_name_range(name, range);

        for node in traversal.filter(|x| x.kind() == "extended_feature_name") {
            let feature =
                Feature::from_name_and_range(src[node.byte_range()].into(), node.range().into());
            class.add_feature(feature);
        }
        class.add_location(src);
        class
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_base_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
    class A
    note
    end
        ";
        let tree = parser.parse(src, None).expect("AST");

        let class = Class::from_tree_and_src(&tree, &src);

        assert_eq!(
            class.name(),
            "A".to_string(),
            "Equality of {} and {}",
            class.name(),
            "A".to_string()
        );
    }

    #[test]
    fn process_annotated_class() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    ";
        let tree = parser.parse(src, None).expect("AST");

        let class = Class::from_tree_and_src(&tree, &src);

        assert_eq!(class.name(), "DEMO_CLASS".to_string());
    }

    #[test]
    fn process_procedure() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";
        let tree = parser.parse(src, None).unwrap();
        let class = Class::from_tree_and_src(&tree, &src);
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "f".to_string());
    }

    #[test]
    fn process_attribute() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let src = "
class A
feature
    x: INTEGER
end
";
        let tree = parser.parse(src, None).unwrap();

        let class = Class::from_tree_and_src(&tree, &src);
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "x".to_string());
    }
}
