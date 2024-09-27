use crate::lib::code_entities::{Class, Feature, Point, Range};

use anyhow::anyhow;
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

impl<'a> TryFrom<(&Tree, &'a str)> for Class {
    type Error = anyhow::Error;

    fn try_from((tree, src): (&Tree, &'a str)) -> Result<Self, Self::Error> {
        let cursor = tree.walk();
        let mut traversal = WidthFirstTraversal::new(cursor);

        let node = traversal
            .find(|x| x.kind() == "class_name")
            .expect("class_name");

        let name = src[node.byte_range()].into();
        let range = node.range().into();
        let mut class = Self::from_name_range(name, range);

        for node in traversal.filter(|x| x.kind() == "feature_declaration") {
            let range = node.range().into();
            let mut cursor = tree.walk();
            cursor.reset(node);
            let mut traversal = WidthFirstTraversal::new(cursor);
            let name = src[traversal
                .find(|x| x.kind() == "extended_feature_name")
                .expect("Each feature declaration contains an extended feature name")
                .byte_range()]
            .into();
            let feature = Feature::from_name_and_range(name, range);
            class.add_feature(&feature);
        }
        Ok(class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::processed_file::ProcessedFile;
    use crate::lib::tree_sitter::WidthFirstTraversal;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

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

        let class = Class::try_from((&tree, src)).expect("Parse class");

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

        let class = Class::try_from((&tree, src)).expect("Parse class");

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
        let class = Class::try_from((&tree, src)).expect("Parse class");
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

        let class = Class::try_from((&tree, src)).expect("Parse class");
        let features = class.features().clone();

        assert_eq!(class.name(), "A".to_string());
        assert_eq!(features.first().unwrap().name(), "x".to_string());
    }
    #[test]
    fn width_first_traversal() -> std::io::Result<()> {
        let procedure_src: &str = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";
        let procedure_path: &str = "/tmp/class_with_feature_path.e";
        let procedure_path: PathBuf = PathBuf::from(procedure_path);
        let mut file = File::create(&procedure_path)?;
        file.write_all(procedure_src.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, procedure_path.clone());

        let cursor = file.tree.walk();
        let mut width_first = WidthFirstTraversal::new(cursor);

        assert_eq!(
            width_first.next().expect("source file node").kind(),
            "source_file"
        );
        assert_eq!(
            width_first.next().expect("class declaration node").kind(),
            "class_declaration"
        );
        assert_eq!(width_first.next().expect("class").kind(), "class");
        assert_eq!(width_first.next().expect("class_name").kind(), "class_name");
        assert_eq!(
            width_first.next().expect("feature clause").kind(),
            "feature_clause"
        );

        Ok(())
    }
}
