use super::*;

trait FeatureTree: Nodes {
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
                )? @attribute_or_routine)* @feature)*
            "#,
        )
    }
    fn names(&mut self) -> Result<Vec<Node<'_>>, Self::Error> {
        self.nodes("feature_name")
    }
    fn arguments(&mut self) -> Result<Option<Node<'_>>, Self::Error> {
        let mut nodes = self.nodes("parameters")?;
        assert!(nodes.len() < 2);
        Ok(nodes.pop())
    }
    fn return_type(&mut self) -> Result<Option<Node<'_>>, Self::Error> {
        let mut nodes = self.nodes("return_type")?;
        assert!(nodes.len() < 2);
        Ok(nodes.pop())
    }
    fn notes(&mut self) -> Result<Option<Node<'_>>, Self::Error> {
        let mut nodes = self.nodes("notes")?;
        assert!(nodes.len() < 2);
        Ok(nodes.pop())
    }
    fn is_routine_or_lazy_initialized(&mut self) -> Result<bool, Self::Error> {
        Ok(self.nodes("attribute_or_routine")?.is_empty())
    }
    fn preconditions(&mut self) -> Result<Option<Node<'_>>, Self::Error> {
        let mut nodes = self.nodes("precondition")?;
        assert!(nodes.len() < 2);
        Ok(nodes.pop())
    }
    fn postcondition(&mut self) -> Result<Option<Node<'_>>, Self::Error> {
        let mut nodes = self.nodes("postcondition")?;
        assert!(nodes.len() < 2);
        Ok(nodes.pop())
    }
}

impl<T: Nodes> FeatureTree for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::util::TreeTraversal;

    impl TreeTraversal<'_, '_> {
        fn mock_feature() -> Self {
            todo!()
        }
    }
    #[test]
    fn names() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.names()));
    }
    #[test]
    fn arguments() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.arguments()));
    }
    #[test]
    fn return_type() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.return_type()));
    }
    #[test]
    fn notes() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.notes()));
    }
    #[test]
    fn is_routine_or_lazy_initialized() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.is_routine_or_lazy_initialized()));
    }
    #[test]
    fn preconditions() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.preconditions()));
    }
    #[test]
    fn postcondition() {
        let mut feature_tree = TreeTraversal::mock_feature();
        let valid = |val| -> bool { todo!() };
        assert!(valid(feature_tree.postcondition()));
    }
}
