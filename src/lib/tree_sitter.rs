use std::ops::{Deref, DerefMut};
use tree_sitter::{Node, Tree, TreeCursor};

struct WidthFirstTraversal<'a> {
    cursor: TreeCursor<'a>,
    stack: Vec<Node<'a>>,
}

impl<'a> Deref for WidthFirstTraversal<'a> {
    type Target = TreeCursor<'a>;
    fn deref(&self) -> &Self::Target {
        return &self.cursor;
    }
}
impl<'a> DerefMut for WidthFirstTraversal<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        return &mut self.cursor;
    }
}

impl<'a> Iterator for WidthFirstTraversal<'a> {
    type Item = Node<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            let node = self.node();
            self.stack.push(node);
            return Some(node);
        }
        if self.goto_next_sibling() {
            let node = self.node();
            self.stack.push(node);
            return Some(node);
        } else {
            loop {
                let cursor = &mut self.cursor;
                cursor.reset(self.stack.pop().expect("One node in the stack here"));
                if cursor.goto_first_child() {
                    let node = self.node();
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
