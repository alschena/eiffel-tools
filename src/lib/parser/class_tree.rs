use crate::lib::parser::util::Nodes;
use crate::lib::parser::*;

mod feature_tree;
mod inheritance_tree;
mod notes_tree;

pub trait ClassTree: Nodes {
    fn query() -> Query {
        util::query(
            r#"
            (class_declaration
                (notes)* @notes
                (class_name) @name
                (inheritance)* @inheritance
                (feature_clause)* @feature_clause
            )@class
                
            "#,
        )
    }
    fn class_name(&mut self) -> Result<Node<'_>, Self::Error> {
        let mut nodes = self.nodes("name")?;
        assert_eq!(nodes.len(), 1);
        Ok(nodes.pop().unwrap())
    }
    fn inheritance(&mut self) -> Result<Vec<Node<'_>>, Self::Error> {
        self.nodes("inheritance")
    }
    fn feature_clauses(&mut self) -> Result<Vec<Node<'_>>, Self::Error> {
        self.nodes("feature_clause")
    }
}

impl<T> ClassTree for T where T: Nodes {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::tests::*;
    use crate::lib::parser::util::TreeTraversal;

    impl<'tree> TreeTraversal<'_, 'tree> {
        fn mock_class(tree: &'tree Tree) -> Self {
            TreeTraversal::try_new(
                DOUBLE_FEATURE_CLASS_SOURCE.as_bytes(),
                tree.root_node(),
                <TreeTraversal as ClassTree>::query(),
            )
            .unwrap_or_else(|e| panic!("{e}"))
        }
    }

    #[test]
    fn class_name_node() -> anyhow::Result<()> {
        let mut tree = Parser::new().mock_tree();
        let mut class_tree = TreeTraversal::mock_class(&tree);

        let name = class_tree.class_name()?;
        assert_eq!(name, todo!());
        Ok(())
    }

    #[test]
    fn class_features_nodes() -> anyhow::Result<()> {
        let mut tree = Parser::new().mock_tree();
        let mut class_tree = TreeTraversal::mock_class(&tree);

        let features_clause = class_tree.feature_clauses()?;
        assert!(features_clause.contains(todo!()));
        Ok(())
    }

    #[test]
    fn inheritance() -> anyhow::Result<()> {
        let mut tree = Parser::new().mock_tree();
        let mut class_tree = TreeTraversal::mock_class(&tree);

        let inheritance_tree = class_tree.inheritance()?;
        assert!(inheritance_tree.contains(todo!()));
        Ok(())
    }
}
