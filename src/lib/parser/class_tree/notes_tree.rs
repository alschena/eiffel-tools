use crate::lib::parser::util::is_inside;
use crate::lib::parser::util::Traversal;
use crate::lib::parser::*;
use anyhow::Result;

pub trait NotesTree<'source, 'tree> {
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

    fn goto_notes_tree(&mut self, node: Node<'tree>);

    fn notes(&mut self) -> Result<FeatureNotes>;

    fn notes_nodes(&mut self) -> Result<Vec<(Node<'tree>, Vec<Node<'tree>>)>>;

    fn model_nodes(&mut self) -> Result<Vec<Node<'tree>>>;

    fn model_names(&mut self) -> Result<Vec<&str>>;
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> NotesTree<'source, 'tree> for T {
    fn goto_notes_tree(&mut self, node: Node<'tree>) {
        assert_eq!(node.kind(), "notes");
        self.set_node_and_query(node, <Self as NotesTree>::query());
    }

    fn notes(&mut self) -> Result<FeatureNotes> {
        assert_eq!(self.current_node().kind(), "notes");
        let nodes = self.notes_nodes()?;
        let note_entries = nodes
            .iter()
            .map(|&(name, ref values)| -> Result<_> {
                let name = self.node_content(name)?.to_string();
                let values: Vec<String> = values
                    .iter()
                    .map(|&val| self.node_content(val).map(|content| content.to_string()))
                    .collect::<Result<Vec<_>>>()?;
                Ok((name, values))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(FeatureNotes(note_entries))
    }

    fn notes_nodes(&mut self) -> Result<Vec<(Node<'tree>, Vec<Node<'tree>>)>> {
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

    fn model_nodes(&mut self) -> Result<Vec<Node<'tree>>> {
        self.nodes_captures("model_value")
    }

    fn model_names(&mut self) -> Result<Vec<&str>> {
        self.model_nodes()?
            .into_iter()
            .map(|node| self.node_content(node))
            .collect::<Result<Vec<_>>>()
    }
}

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

    impl<'source, 'tree: 'source> TreeTraversal<'source, 'tree> {
        fn mock_model(parsed_source: &'tree ParsedSource<'source>) -> anyhow::Result<Self> {
            let mut tree_traversal = parsed_source.class_tree_traversal()?;
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
