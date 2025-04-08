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
use tracing::warn;

mod parameters;
use parameters::ParameterTree;

pub trait FeatureClauseTree<'source, 'tree>: FeatureTree<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
            (feature_clause (feature_declaration)* @feature)
            "#,
        )
    }

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
                    self.clause_features().inspect_err(|e| {
                    warn!("fails to parse feature clause at node: {feature_declaration_node:#?} with error: {e:#?}")
                }).ok()
                }).fold(Vec::new(),|mut acc, mut features|
                {acc.append(&mut features); acc});

        Ok(features)
    }
}

impl<'source, 'tree, T: FeatureTree<'source, 'tree>> FeatureClauseTree<'source, 'tree> for T {}

trait FeatureTree<'source, 'tree>:
    NotesTree<'source, 'tree>
    + ContractTree<'source, 'tree>
    + EiffelTypeTree<'source, 'tree>
    + ParameterTree<'source, 'tree>
{
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

    fn goto_feature_tree(&mut self, feature_declaration_node: Node<'tree>) {
        assert_eq!(feature_declaration_node.kind(), "feature_declaration");
        self.set_node_and_query(feature_declaration_node, <Self as FeatureTree>::query());
    }

    fn clause_features(&mut self) -> Result<Vec<Feature>, Self::Error> {
        let outer_node = self.nodes_captures("feature_declaration")?;
        let names = self.nodes_captures("feature_name")?;
        let parameters = self.nodes_captures("parameters")?;
        let return_type = self.nodes_captures("return_type")?;
        let notes = self.nodes_captures("notes")?;
        let preconditions = self.nodes_captures("precondition")?;
        let postconditions = self.nodes_captures("postcondition")?;

        let names = names
            .iter()
            .map(|name| self.node_content(*name).map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        let parameters = parameters
            .first()
            .map(|parameters_node| -> Result<_, Self::Error> {
                self.goto_parameter_tree(*parameters_node);
                self.parameters()
            })
            .transpose()?
            .unwrap_or_default();

        let return_type = return_type
            .first()
            .map(|type_node| {
                self.goto_eiffel_type_tree(*type_node);
                self.eiffel_type()
            })
            .transpose()?;

        let notes = notes
            .first()
            .map(|&note_node| -> Result<_, Self::Error> {
                self.goto_notes_tree(note_node);
                self.notes()
            })
            .transpose()?;

        let range: Range = outer_node
            .first()
            .map(|outer| outer.range())
            .with_context(|| "fails to get feature declaration.")?
            .into();

        let preconditions = preconditions
            .first()
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
            .first()
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

impl<'source, 'tree, T> FeatureTree<'source, 'tree> for T where
    T: NotesTree<'source, 'tree>
        + ContractTree<'source, 'tree>
        + EiffelTypeTree<'source, 'tree>
        + ParameterTree<'source, 'tree>
{
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
                .set_node_and_query(first_feature, <TreeTraversal as FeatureClauseTree>::query());
            Ok(tree_traversal)
        }
    }

    fn extracted_features(parsed_source: &ParsedSource) -> anyhow::Result<Vec<Feature>> {
        let mut feature_tree = TreeTraversal::mock_feature(&parsed_source)?;
        feature_tree.features()
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
