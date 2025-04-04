use crate::lib::parser::util::TreeTraversal;
use crate::lib::parser::*;

trait NotesTree<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
            (notes (note_entry
                (tag) @class_note_tag
                value: (_) @class_note_value_id
                ("," value: (_) @class_note_value_id)*)
                (#eq? @class_note_tag "model") )*
            "#,
        )
    }
    fn model(&self) -> Vec<Node<'tree>>;
}

impl<'source, 'tree> NotesTree<'source, 'tree> for TreeTraversal<'source, 'tree> {
    fn model(&self) -> Vec<Node<'tree>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    impl TreeTraversal<'_, '_> {
        fn mock_model() -> Self {
            todo!()
        }
    }

    #[test]
    fn model() {
        let tree = TreeTraversal::mock_model();
        let valid = |val| -> bool { todo!() };
        assert!(valid(tree.model()));
    }
}
