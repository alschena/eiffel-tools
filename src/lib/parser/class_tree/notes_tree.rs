use crate::lib::parser::util::is_inside;
use crate::lib::parser::util::Traversal;
use crate::lib::parser::*;

pub trait NotesTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
            (notes (note_entry
                (tag) @model_tag
                value: (_) @model_value
                ("," value: (_) @model_value)*)
                (#eq? @model_tag "model") )
            (notes (note_entry
                (tag) @note_tag
                value: (_) @note_value
                ("," value: (_) @note_value)*))
            "#,
        )
    }

    fn goto_notes_tree(&mut self, node: Node<'tree>) {
        assert_eq!(node.kind(), "notes");
        self.set_node_and_query(node, <Self as NotesTree>::query());
    }

    fn notes(&mut self) -> Result<FeatureNotes, Self::Error> {
        assert_eq!(self.current_node().kind(), "notes");
        let nodes = self.notes_nodes()?;
        let note_entries = nodes
            .iter()
            .map(|&(name, ref values)| -> Result<_, Self::Error> {
                let name = self.node_content(name)?.to_string();
                let values: Vec<String> = values
                    .iter()
                    .map(|&val| self.node_content(val).map(|content| content.to_string()))
                    .collect::<Result<Vec<_>, Self::Error>>()?;
                Ok((name, values))
            })
            .collect::<Result<Vec<_>, Self::Error>>()?;
        Ok(FeatureNotes(note_entries))
    }

    fn notes_nodes(&mut self) -> Result<Vec<(Node<'tree>, Vec<Node<'tree>>)>, Self::Error> {
        let tags = self.nodes_captures("note_tag")?;
        let values = self.nodes_captures("note_value")?;

        Ok(tags
            .iter()
            .map(|&tag_node| {
                let value_nodes = values
                    .iter()
                    .filter(|&&value_node| {
                        tag_node
                            .parent()
                            .is_some_and(|parent| is_inside(value_node, parent))
                    })
                    .cloned()
                    .collect::<Vec<Node>>();
                (tag_node, value_nodes)
            })
            .collect())
    }

    fn model_nodes(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("model_value")
    }

    fn model_names(&mut self) -> Result<Vec<&str>, Self::Error> {
        self.model_nodes()?
            .into_iter()
            .map(|node| self.node_content(node))
            .collect::<Result<Vec<_>, Self::Error>>()
    }
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> NotesTree<'source, 'tree> for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::util::TreeTraversal;
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
                .class_notes()?
                .pop()
                .with_context(|| "fails to get the class notes node.")?;
            tree_traversal.set_node_and_query(node, <TreeTraversal as NotesTree>::query());
            Ok(tree_traversal)
        }
    }

    #[test]
    fn model() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(MODEL_CLASS_SOURCE)?;
        let mut model_tree = TreeTraversal::mock_model(&parsed_source)?;
        let mut model = model_tree.model_nodes()?;
        let model_feature = model.pop().with_context(|| "fails to get model feature")?;
        assert_eq!(model_tree.node_content(model_feature)?, "seq");
        assert!(model.pop().is_none());
        Ok(())
    }
}
