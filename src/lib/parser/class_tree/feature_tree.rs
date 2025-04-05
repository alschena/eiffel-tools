use super::*;

trait FeatureTree<'source, 'tree>: Nodes<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
            (feature_clause (feature_declaration
                (new_feature (extended_feature_name) @feature_name)
                ("," (new_feature (extended_feature_name) @feature_name))*
                (formal_arguments)? @parameters
                type: (_)? @return_type
                (attribute_or_routine
                    (notes)? @notes
                    (precondition
                        (assertion_clause (expression))* @assertion_clause)? @precondition
                    (postcondition
                        (assertion_clause (expression))* @assertion_clause)? @postcondition
                )? @attribute_or_routine)* @feature)
            "#,
        )
    }
    fn features(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("feature")
    }
    fn names(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("feature_name")
    }
    fn arguments(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("parameters")
    }
    fn return_type(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("return_type")
    }
    fn notes(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("notes")
    }
    fn preconditions(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("precondition")
    }
    fn postcondition(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("postcondition")
    }
}

impl<'source, 'tree, T: Nodes<'source, 'tree>> FeatureTree<'source, 'tree> for T {}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::{tests::DOUBLE_FEATURE_CLASS_SOURCE, util::TreeTraversal};

    impl<'source, 'tree> TreeTraversal<'source, 'tree> {
        pub fn mock_feature<'tmp_src: 'tree>(
            parsed_file: &'tmp_src ParsedSource<'source>,
        ) -> anyhow::Result<Self> {
            let mut tree_traversal = TreeTraversal::try_from(parsed_file)?;
            let mut features = tree_traversal.feature_clauses()?;
            let first_feature = features.pop().with_context(|| {
                "fails to get a feature to create the mock feature tree traversal."
            })?;
            tree_traversal.goto_node(first_feature);
            tree_traversal.set_query(<TreeTraversal as FeatureTree>::query());
            Ok(tree_traversal)
        }
    }

    #[test]
    fn features() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        let features = feature_tree.features()?;
        assert_eq!(features.len(), 2);
        Ok(())
    }

    #[test]
    fn names() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        let mut names = feature_tree.names()?;
        assert_eq!(names.len(), 2, "names node: {names:#?}");
        assert_eq!(feature_tree.node_content(names.pop().unwrap())?, "y");
        assert_eq!(feature_tree.node_content(names.pop().unwrap())?, "x");
        Ok(())
    }

    #[test]
    fn arguments() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.arguments()?.is_empty());
        Ok(())
    }

    #[test]
    fn return_type() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        let mut ret_types = feature_tree.return_type()?;
        assert_eq!(
            feature_tree.node_content(ret_types.pop().unwrap())?,
            "INTEGER"
        );
        assert_eq!(
            feature_tree.node_content(ret_types.pop().unwrap())?,
            "INTEGER"
        );
        Ok(())
    }

    #[test]
    fn notes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.notes()?.is_empty());
        Ok(())
    }

    #[test]
    fn preconditions() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.preconditions()?.is_empty());
        Ok(())
    }

    #[test]
    fn postcondition() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.postcondition()?.is_empty());
        Ok(())
    }
}
