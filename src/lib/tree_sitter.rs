pub use tree_sitter::{Node, Tree, TreeCursor};

pub(crate) struct WidthFirstTraversal<'a, 'b> {
    cursor: &'b mut TreeCursor<'a>,
    stack: Vec<Node<'a>>,
}

impl<'a, 'b> WidthFirstTraversal<'a, 'b> {
    pub(crate) fn new(cursor: &'b mut TreeCursor<'a>) -> WidthFirstTraversal<'a, 'b> {
        let stack = Vec::new();
        WidthFirstTraversal { cursor, stack }
    }
}

impl<'a, 'b> Iterator for WidthFirstTraversal<'a, 'b> {
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
pub trait ExtractedFrom: Sized {
    type Error;
    fn extract_from(node: &Node, src: &str) -> Result<Self, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::processed_file::ProcessedFile;
    use anyhow::Result;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;
    #[test]
    fn width_first_traversal() -> Result<()> {
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
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, procedure_path.clone())?;

        let mut cursor = file.tree.walk();
        let mut width_first = WidthFirstTraversal::new(&mut cursor);

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
