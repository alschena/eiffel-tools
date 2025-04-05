use crate::lib::parser::util::Nodes;
use crate::lib::parser::util::TreeTraversal;
use crate::lib::parser::*;

trait NotesTree<'source, 'tree>: Nodes<'source, 'tree> {
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
    fn model(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("class_note_value_id")
    }
}

impl<'source, 'tree, T: Nodes<'source, 'tree>> NotesTree<'source, 'tree> for T {}

#[cfg(test)]
mod tests {
    use super::*;
    pub const MODEL_CLASS_SOURCE: &str = r#"
note
    model: seq
class A
feature
    x: INTEGER
    seq: MML_SEQUENCE [INTEGER]
end
"#;

    impl<'source, 'tree> TreeTraversal<'source, 'tree> {
        fn mock_model(parsed_source: &'tree ParsedSource<'source>) -> anyhow::Result<Self> {
            let mut tree_traversal = TreeTraversal::try_from(parsed_source)?;
            let node = tree_traversal
                .class_nodes()?
                .pop()
                .with_context(|| "fails to get the class notes node.")?;
            tree_traversal.goto_node(node);
            tree_traversal.set_query(<TreeTraversal as NotesTree>::query());
            Ok(tree_traversal)
        }
    }

    #[test]
    fn model() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(MODEL_CLASS_SOURCE)?;
        let mut model_tree = TreeTraversal::mock_model(&parsed_source)?;
        let mut model = model_tree.model()?;
        let model_feature = model.pop().with_context(|| "fails to get model feature")?;
        assert_eq!(model_tree.node_content(model_feature)?, "seq");
        assert!(model.pop().is_none());
        Ok(())
    }
}
