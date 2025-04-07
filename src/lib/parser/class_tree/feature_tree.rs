use super::*;
use crate::lib::code_entities::contract::Block;
use crate::lib::code_entities::prelude::Feature;
use crate::lib::code_entities::prelude::FeatureParameters;
use crate::lib::code_entities::prelude::FeatureVisibility;
use crate::lib::parser::class_tree::contract_tree::ContractTree;
use crate::lib::parser::class_tree::eiffel_type::EiffelTypeTree;
use crate::lib::parser::class_tree::notes_tree::NotesTree;
use crate::lib::parser::contract::Postcondition;
use crate::lib::parser::contract::Precondition;
use crate::lib::parser::util::is_inside;

pub trait FeatureTree<'source, 'tree>:
    NotesTree<'source, 'tree> + ContractTree<'source, 'tree> + EiffelTypeTree<'source, 'tree>
{
    fn query() -> Query {
        util::query(
            r#"
            (feature_clause (feature_declaration
                (new_feature (extended_feature_name) @feature_name)
                ("," (new_feature (extended_feature_name) @feature_name))*
                (formal_arguments
                    (entity_declaration_group
                                    (identifier) @parameter_name
                                    ("," (identifier) @parameter_name)*
                                    type: (_) @parameter_type)
                )? @parameters
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

    fn goto_feature_tree(&mut self, feature_clause_node: Node<'tree>) {
        assert_eq!(feature_clause_node.kind(), "feature_clause");
        self.set_node_and_query(feature_clause_node, <Self as FeatureTree>::query());
    }

    fn features(&mut self) -> Result<Vec<Feature>, Self::Error> {
        let feature_nodes = self.features_nodes()?;
        let names = self.features_names()?;
        let parameters = self.parameters()?;
        let parameter_names = self.nodes_captures("parameter_name")?;
        let parameter_types = self.nodes_captures("parameter_type")?;
        let return_type = self.return_type()?;
        let notes = FeatureTree::notes(self)?;
        let preconditions = self.preconditions()?;
        let postconditions = self.postconditions()?;

        feature_nodes
            .iter()
            .map(|&outer| {
                let name = names.iter().find(|&&inner| is_inside(inner, outer)).expect(
                    format!("fails to find feature name for feature node: {outer}").as_str(),
                );
                let name = self.node_content(*name)?.to_string();

                let parameters = parameters
                    .iter()
                    .find(|&&inner| is_inside(inner, outer))
                    .map(|&outer| -> Result<_, Self::Error> {
                        let names = parameter_names
                            .iter()
                            .filter(|&&inner| is_inside(inner, outer))
                            .map(|&name_node| self.node_content(name_node).unwrap().to_string())
                            .collect::<Vec<_>>();
                        let types = parameter_types
                            .iter()
                            .filter(|&&inner| is_inside(inner, outer))
                            .map(|&type_node| -> Result<_, Self::Error> {
                                self.goto_eiffel_type_tree(type_node);
                                self.eiffel_type()
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(FeatureParameters { names, types })
                    })
                    .transpose()?
                    .unwrap_or_default();

                let return_type = return_type
                    .iter()
                    .find(|&&inner| is_inside(inner, outer))
                    .map(|&type_node| {
                        self.goto_eiffel_type_tree(type_node);
                        self.eiffel_type()
                    })
                    .transpose()?;

                let notes = notes
                    .iter()
                    .find(|&&inner| is_inside(inner, outer))
                    .map(|&note_node| -> Result<_, Self::Error> {
                        self.goto_notes_tree(note_node);
                        <Self as NotesTree>::notes(self)
                    })
                    .transpose()?;
                let range = outer.range().into();
                let preconditions = preconditions
                    .iter()
                    .find(|&&inner| is_inside(inner, outer))
                    .map(|&precondition_node| -> Result<_, Self::Error> {
                        self.goto_contract_tree(precondition_node);
                        let clauses = self.clauses()?;
                        let precondition = Precondition(clauses);
                        Ok(Block {
                            item: precondition,
                            range: precondition_node.range().into(),
                        })
                    })
                    .transpose()?;
                let postconditions = postconditions
                    .iter()
                    .find(|&&inner| is_inside(inner, outer))
                    .map(|&postcondition_node| -> Result<_, Self::Error> {
                        self.goto_contract_tree(postcondition_node);
                        let clauses = self.clauses()?;
                        let postcondition = Postcondition(clauses);
                        Ok(Block {
                            item: postcondition,
                            range: postcondition_node.range().into(),
                        })
                    })
                    .transpose()?;
                Ok::<_, Self::Error>(Feature {
                    name,
                    parameters,
                    return_type,
                    notes,
                    visibility: FeatureVisibility::Private,
                    range,
                    preconditions,
                    postconditions,
                })
            })
            .collect()
    }

    fn features_nodes(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("feature")
    }

    fn features_names(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("feature_name")
    }

    fn parameters(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
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

    fn postconditions(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("postcondition")
    }
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> FeatureTree<'source, 'tree> for T {}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::class_tree::tests::DOUBLE_ATTRIBUTE_CLASS;
    use crate::lib::parser::util::TreeTraversal;

    const CONTRACT_FEATURE_CLASS_SOURCE: &str = r#"
class A feature
  x
    require
      True
    do
    ensure
      True
    end
end"#;

    const NOTES_FEATURE_CLASS_SOURCE: &str = r#"
class A feature
  x
    note
        entry_tag: entry_value
    do
    end
end
        "#;

    const PARAMETERS_AND_RETURN_TYPE_FEATURE_CLASS_SOURCE: &str = r#"
class A feature
  x (y, z: MML_SEQUENCE [INTEGER]): MML_SEQUENCE [INTEGER]
    do
    end
end"#;

    impl<'source, 'tree> TreeTraversal<'source, 'tree> {
        pub fn mock_feature<'tmp_src: 'tree>(
            parsed_file: &'tmp_src ParsedSource<'source>,
        ) -> anyhow::Result<Self> {
            let mut tree_traversal = TreeTraversal::try_from(parsed_file)?;
            let mut features = tree_traversal.feature_clauses()?;
            let first_feature = features.pop().with_context(|| {
                "fails to get a feature to create the mock feature tree traversal."
            })?;
            tree_traversal
                .set_node_and_query(first_feature, <TreeTraversal as FeatureTree>::query());
            Ok(tree_traversal)
        }
    }

    fn extracted_features(parsed_source: &ParsedSource) -> anyhow::Result<Vec<Feature>> {
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        feature_tree.features()
    }

    #[test]
    fn features_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        let features = feature_tree.features_nodes()?;
        assert_eq!(features.len(), 2);
        Ok(())
    }

    #[test]
    fn names_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        let mut names = feature_tree.features_names()?;
        assert_eq!(names.len(), 2, "names node: {names:#?}");
        assert_eq!(feature_tree.node_content(names.pop().unwrap())?, "y");
        assert_eq!(feature_tree.node_content(names.pop().unwrap())?, "x");
        Ok(())
    }

    #[test]
    fn arguments_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.parameters()?.is_empty());
        Ok(())
    }

    #[test]
    fn return_type_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
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
    fn notes_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(FeatureTree::notes(&mut feature_tree)?.is_empty());
        Ok(())
    }

    #[test]
    fn preconditions_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.preconditions()?.is_empty());
        Ok(())
    }

    #[test]
    fn postcondition_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        assert!(feature_tree.postconditions()?.is_empty());
        Ok(())
    }

    #[test]
    fn parse_feature_with_contracts() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(CONTRACT_FEATURE_CLASS_SOURCE)?;
        let mut features = extracted_features(&parsed_source)?;

        let feature = features.pop().with_context(|| {
            "fails to pop feature in source: {CONTRACT_DOUBLE_FEATURE_CLASS_SOURCE}"
        })?;
        assert_eq!(feature.name(), "x");

        let feature_precondition = feature
            .preconditions()
            .with_context(|| "fails to get preconditions of feature: {feature:#?}")?;
        let precondition_clause = feature_precondition
            .first()
            .expect("fails to get first precondition clause.");
        let feature_postcondition = feature
            .postconditions()
            .with_context(|| "fails to get postconditions of feature: {feature:#?}")?;
        let postcondition_clause = feature_postcondition
            .first()
            .expect("fails to get first postcondition clause.");

        assert_eq!(feature_precondition.len(), 1);
        assert_eq!(feature_postcondition.len(), 1);

        assert_eq!(precondition_clause.predicate.as_str(), "True");
        assert_eq!(postcondition_clause.predicate.as_str(), "True");
        Ok(())
    }

    #[test]
    fn parse_notes_feature() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(NOTES_FEATURE_CLASS_SOURCE)?;
        let mut features = extracted_features(&parsed_source)?;
        let feature = features
            .pop()
            .with_context(|| format!("fails to get feature from {NOTES_FEATURE_CLASS_SOURCE}"))?;
        let notes = feature
            .notes
            .with_context(|| format!("fails to get notes from {NOTES_FEATURE_CLASS_SOURCE}"))?;
        let (tag, value) = notes
            .first()
            .with_context(|| "fails to get note entries.")?;
        assert_eq!(tag, "entry_tag", "notes: {notes:#?}");
        assert_eq!(value.first().unwrap(), "entry_value", "notes: {notes:#?}");
        Ok(())
    }

    #[test]
    fn parse_return_type() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(PARAMETERS_AND_RETURN_TYPE_FEATURE_CLASS_SOURCE)?;
        let mut features = extracted_features(&parsed_source)?;
        let feature = features.pop().with_context(|| {
            format!("fails to get feature from {PARAMETERS_AND_RETURN_TYPE_FEATURE_CLASS_SOURCE}")
        })?;

        let return_type = feature.return_type().with_context(||format!("fails to get return type from only feature of source: {PARAMETERS_AND_RETURN_TYPE_FEATURE_CLASS_SOURCE}"))?;
        assert_eq!(
            format!("{return_type}"),
            "MML_SEQUENCE [INTEGER]".to_string()
        );
        Ok(())
    }
}
