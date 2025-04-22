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
use tracing::warn;

mod parameter_tree;
use parameter_tree::ParameterTree;

pub trait FeatureClauseTree<'source, 'tree> {
    type Error;

    fn query() -> Query {
        util::query(
            r#"
            (feature_clause (feature_declaration)* @feature)
            "#,
        )
    }

    fn goto_feature_clause_tree(&mut self, feature_clause_node: Node<'tree>);

    fn features(&mut self) -> Result<Vec<Feature>>;
}

impl<'source, 'tree, T: FeatureTree<'source, 'tree>> FeatureClauseTree<'source, 'tree> for T {
    type Error = anyhow::Error;
    fn goto_feature_clause_tree(&mut self, feature_clause_node: Node<'tree>) {
        assert_eq!(feature_clause_node.kind(), "feature_clause");
        self.set_node_and_query(feature_clause_node, <Self as FeatureClauseTree>::query());
    }

    fn features(&mut self) -> Result<Vec<Feature>, Self::Error> {
        let feature_declaration = self.nodes_captures("feature")?;

        let features: Vec<Feature> =
            feature_declaration
                .iter()
                .filter_map(|feature_declaration_node| {
                    self.goto_feature_tree(*feature_declaration_node);
                    self.feature().inspect_err(|e| {
                    warn!("fails to parse feature clause at node: {feature_declaration_node:#?} with error: {e:#?}")
                }).ok()
                }).fold(Vec::new(),|mut acc, mut features|
                {acc.append(&mut features); acc});

        Ok(features)
    }
}

pub trait FeatureTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"(feature_declaration
                (new_feature (extended_feature_name) @feature_name)
                ("," (new_feature (extended_feature_name) @feature_name))*
                (formal_arguments)? @parameters
                type: (_)? @return_type
                (attribute_or_routine
                    (notes)? @notes
                    (precondition)? @precondition
                    (postcondition)? @postcondition
                )? @attribute_or_routine) @feature_declaration"#,
        )
    }

    fn goto_feature_tree(&mut self, feature_declaration_node: Node<'tree>);

    fn feature(&mut self) -> Result<Vec<Feature>>;
}

impl<'source, 'tree> FeatureTree<'source, 'tree> for TreeTraversal<'source, 'tree> {
    fn goto_feature_tree(&mut self, feature_declaration_node: Node<'tree>) {
        assert!(
            feature_declaration_node.kind() == "feature_declaration"
                || feature_declaration_node.kind() == "source_file"
        );
        self.set_node_and_query(feature_declaration_node, <Self as FeatureTree>::query());
    }

    fn feature(&mut self) -> Result<Vec<Feature>> {
        let outer_node = self.nodes_captures("feature_declaration")?;
        let names_nodes = self.nodes_captures("feature_name")?;

        let parameters_node = self.nodes_captures("parameters")?;
        let parameters_node = parameters_node.first();

        let return_type_node = self.nodes_captures("return_type")?;
        let return_type_node = return_type_node.first();

        let notes_node = self.nodes_captures("notes")?;
        let notes_node = notes_node.first();

        let preconditions_node = self.nodes_captures("precondition")?;
        let preconditions_node = preconditions_node.first();

        let postconditions_node = self.nodes_captures("postcondition")?;
        let postconditions_node = postconditions_node.first();

        let some_attribute_or_routine_range_if_contracts_supported = self
            .nodes_captures("attribute_or_routine")?
            .first()
            .map(|aor_node| aor_node.range());

        let names = names_nodes
            .iter()
            .map(|name| self.node_content(*name).map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        let parameters = parameters_node
            .map(|parameters_node| -> Result<_> {
                self.goto_parameter_tree(*parameters_node);
                self.parameters()
            })
            .transpose()?
            .unwrap_or_default();

        let return_type = return_type_node
            .map(|type_node| {
                self.goto_eiffel_type_tree(*type_node);
                self.eiffel_type()
            })
            .transpose()?;

        let notes = notes_node
            .map(|&note_node| -> Result<_> {
                self.goto_notes_tree(note_node);
                self.notes()
            })
            .transpose()?;

        let range: Range = outer_node
            .first()
            .map(|outer| outer.range())
            .with_context(|| "fails to get feature declaration.")?
            .into();

        let preconditions: Option<Block<Precondition>> = preconditions_node.map_or_else(
            || -> Result<Option<Block<Precondition>>> {
                Ok(
                    some_attribute_or_routine_range_if_contracts_supported.map(|range| {
                        let point_for_collapsed_block = match notes_node {
                            Some(note_node) => note_node.range().end_point,
                            None => range.start_point,
                        };
                        Block::new_empty(point_for_collapsed_block.into())
                    }),
                )
            },
            |&precondition_node| -> Result<Option<Block<Precondition>>> {
                self.goto_contract_tree(precondition_node);
                let clauses = self.clauses()?;
                let precondition = Precondition(clauses);
                Ok(Some(Block {
                    item: precondition,
                    range: precondition_node.range().into(),
                }))
            },
        )?;

        let postconditions: Option<Block<Postcondition>> = postconditions_node.map_or_else(
            || -> Result<Option<Block<_>>> {
                Ok(
                    some_attribute_or_routine_range_if_contracts_supported.map(|range| {
                        let mut point_of_collapsed_block: Point = range.end_point.into();

                        // Compensates the word `end`.
                        point_of_collapsed_block.shift_left(3);
                        Block::new_empty(point_of_collapsed_block)
                    }),
                )
            },
            |&postcondition_node| -> Result<Option<Block<_>>> {
                self.goto_contract_tree(postcondition_node);
                let clauses = self.clauses()?;
                let postcondition = Postcondition(clauses);
                Ok(Some(Block {
                    item: postcondition,
                    range: postcondition_node.range().into(),
                }))
            },
        )?;

        let features = names
            .iter()
            .map(|name| Feature {
                name: name.to_string(),
                parameters: parameters.clone(),
                return_type: return_type.clone(),
                notes: notes.clone(),
                visibility: FeatureVisibility::Private,
                range: range.clone(),
                preconditions: preconditions.clone(),
                postconditions: postconditions.clone(),
            })
            .collect();
        Ok(features)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::class_tree::tests::DOUBLE_ATTRIBUTE_CLASS;
    use crate::lib::parser::util::TreeTraversal;

    const CONTRACT_FEATURE_CLASS_SOURCE: &str = r#"
class A feature
  x
    require
      tag_1: True
    do
    ensure
      True
    end
  y
    require else
      True
    do
    ensure then
      tag_2: True
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
        pub fn mock_feature<'tmp_src: 'source + 'tree>(
            parsed_file: &'tmp_src ParsedSource<'source>,
        ) -> anyhow::Result<Self> {
            let mut tree_traversal = parsed_file.class_tree_traversal()?;
            let mut features = tree_traversal.feature_clauses()?;
            let first_feature = features.pop().with_context(|| {
                "fails to get a feature to create the mock feature tree traversal."
            })?;
            tree_traversal
                .set_node_and_query(first_feature, <TreeTraversal as FeatureClauseTree>::query());
            Ok(tree_traversal)
        }
    }

    pub fn extracted_features(parsed_source: &ParsedSource) -> anyhow::Result<Vec<Feature>> {
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        feature_tree.features()
    }

    #[test]
    fn parse_feature_with_contracts() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(CONTRACT_FEATURE_CLASS_SOURCE)?;
        let features = extracted_features(&parsed_source)?;

        let first_feature = &features[0];
        let second_feature = &features[1];

        assert_eq!(first_feature.name(), "x");
        assert_eq!(second_feature.name(), "y");

        let first_feature_precondition = first_feature
            .preconditions()
            .with_context(|| "fails to get preconditions of feature: {feature:#?}")?;
        let first_feature_postcondition = first_feature
            .postconditions()
            .with_context(|| "fails to get postconditions of feature: {feature:#?}")?;
        let second_feature_precondition = second_feature
            .preconditions()
            .with_context(|| "fails to get preconditions from second feature.")?;
        let second_feature_postcondition = second_feature
            .postconditions()
            .with_context(|| "fails to get postconditions from second feature.")?;

        assert_eq!(first_feature_precondition.len(), 1);
        assert_eq!(first_feature_precondition[0].predicate.as_str(), "True");
        assert_eq!(first_feature_precondition[0].tag.as_str(), "tag_1");

        assert_eq!(first_feature_postcondition.len(), 1);
        assert_eq!(first_feature_postcondition[0].predicate.as_str(), "True");
        assert_eq!(first_feature_postcondition[0].tag.as_str(), "");

        assert_eq!(second_feature_precondition.len(), 1);
        assert_eq!(second_feature_precondition[0].predicate.as_str(), "True");
        assert_eq!(second_feature_precondition[0].tag.as_str(), "");

        assert_eq!(second_feature_postcondition.len(), 1);
        assert_eq!(second_feature_postcondition[0].predicate.as_str(), "True");
        assert_eq!(second_feature_postcondition[0].tag.as_str(), "tag_2");

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
