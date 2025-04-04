use crate::lib::parser::util::Nodes;
use crate::lib::parser::*;

mod feature_tree;
mod inheritance_tree;
mod notes_tree;

pub trait ClassTree<'source, 'tree>: Nodes<'source, 'tree> {
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
    fn class_name(&mut self) -> Result<Node<'tree>, Self::Error> {
        let mut nodes = self.nodes("name")?;
        assert_eq!(nodes.len(), 1);
        Ok(nodes.pop().unwrap())
    }
    fn inheritance(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes("inheritance")
    }
    fn feature_clauses(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes("feature_clause")
    }
}

impl<'source, 'tree, T> ClassTree<'source, 'tree> for T where T: Nodes<'source, 'tree> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::tests::*;
    use crate::lib::parser::util::TreeTraversal;

    #[test]
    fn class_name_node() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
        let name = class_tree.class_name()?;
        let content = class_tree.node_content(name)?;
        assert_eq!(content, "TEST");
        Ok(())
    }

    #[test]
    fn class_features_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;

        let mut features_clause = class_tree.feature_clauses()?;
        assert_eq!(
            features_clause.len(),
            1,
            "fails to parse the single feature clause, i.e. feature visibility block."
        );
        let feature_clause_content = class_tree.node_content(features_clause.pop().unwrap())?;
        assert_eq!(
            feature_clause_content.trim(),
            r#"feature
    x: INTEGER
    y: INTEGER"#
        );
        Ok(())
    }

    #[test]
    fn inheritance() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_FEATURE_CLASS_SOURCE)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;

        let inheritance_tree = class_tree.inheritance()?;
        assert!(inheritance_tree.is_empty());
        Ok(())
    }
}
