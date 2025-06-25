use std::sync::LazyLock;

use super::*;
use crate::parser::class_tree::contract_tree::ContractTree;
use crate::parser::class_tree::eiffel_type::EiffelTypeTree;
use crate::parser::class_tree::notes_tree::NotesTree;
use crate::parser::contract::Postcondition;
use crate::parser::contract::Precondition;
use tracing::warn;

mod parameter_tree;
use parameter_tree::ParameterTree;

pub static FEATURE_CLAUSE_QUERY: LazyLock<Query> = LazyLock::new(||
        util::query(
            r#"
            (feature_clause (feature_declaration)* @feature)
            "#,
        )
);

pub static FEATURE_QUERY: LazyLock<Query> = LazyLock::new(||
        util::query(
            r#"(feature_declaration
                (new_feature (extended_feature_name) @feature_name)
                ("," (new_feature (extended_feature_name) @feature_name))*
                (formal_arguments)? @parameters
                type: (_)? @return_type
                (attribute_or_routine
                    (notes)? @notes
                    (precondition)? @precondition
                    (feature_body) @body
                    (postcondition)? @postcondition
                )? @attribute_or_routine) @feature_declaration"#,
        )
);

pub trait FeatureClauseTree<'source, 'tree> {
    type Error;

    fn goto_feature_clause_tree(&mut self, feature_clause_node: Node<'tree>);

    fn features(&mut self) -> Result<Vec<Feature>>;
}

impl<'source, 'tree, T: FeatureTree<'source, 'tree>> FeatureClauseTree<'source, 'tree> for T {
    type Error = anyhow::Error;
    fn goto_feature_clause_tree(&mut self, feature_clause_node: Node<'tree>) {
        assert_eq!(feature_clause_node.kind(), "feature_clause");
        self.set_node_and_query(feature_clause_node, &FEATURE_CLAUSE_QUERY);
    }

    fn features(&mut self) -> Result<Vec<Feature>, Self::Error> {
        let feature_declaration = self.nodes_captures("feature")?;

        Ok(feature_declaration.into_iter().fold(Vec::new(),|mut acc, feature_declaration_node| {
            self.goto_feature_tree(feature_declaration_node);
            match self.feature() {
                Ok(features_in_clause) => acc.extend(features_in_clause),
                Err(e) => {
                    warn!(
                        "fails to parse feature clause at node: {feature_declaration_node:#?} with error: {e:#?}"
                    )},
                }
            acc
            
        }))
    }
}

pub trait FeatureTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn goto_feature_tree(&mut self, feature_declaration_node: Node<'tree>);

    fn feature(&mut self) -> Result<impl IntoIterator<Item = Feature>>;

    fn feature_names(&mut self, feature_nodes: &FeatureNodes<'tree>) -> Result<Vec<String>>;

    fn feature_parameters(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<FeatureParameters>;

    fn feature_return_type(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<EiffelType>>;

    fn feature_precondition(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<Block<Precondition>>>;

    fn feature_postcondition(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<Block<Postcondition>>>;

    fn feature_notes(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<FeatureNotes>>;
}

pub struct FeatureNodes<'tree> {
    names: Vec<Node<'tree>>,
    parameters: Option<Node<'tree>>,
    return_type: Option<Node<'tree>>,
    precondition: Option<Node<'tree>>,
    postcondition: Option<Node<'tree>>,
    notes: Option<Node<'tree>>,
    attribute_or_routine: Option<Node<'tree>>,
    body: Option<Node<'tree>>,
    whole_feature: Node<'tree>,
}

impl<'tree> FeatureNodes<'tree> {
    fn feature_range(&self) -> Range {
        self.whole_feature.range().into()
    }

    fn feature_body_range(&self) -> Option<Range> {
        self.body.map(|body_node| body_node.range().into())
    }
}

impl<'source, 'tree> TreeTraversal<'source, 'tree> {
    fn feature_nodes(&mut self) -> Option<FeatureNodes<'tree>> {
        let initial_node = self.current_node();
        debug_assert!(
            initial_node.kind() == "feature_declaration" || initial_node.kind() == "source_file",
            "initial node kind: {}",
            initial_node.kind()
        );
        let index_feature_name = self.capture_index_of("feature_name");
        let index_parameters = self.capture_index_of("parameters");
        let index_return_type = self.capture_index_of("return_type");
        let index_notes = self.capture_index_of("notes");
        let index_precondition = self.capture_index_of("precondition");
        let index_postcondition = self.capture_index_of("postcondition");
        let index_attribute_or_routine = self.capture_index_of("attribute_or_routine");
        let index_body = self.capture_index_of("body");
        let index_feature_declaration = self.capture_index_of("feature_declaration");

        let mut names: Vec<Node<'tree>> = Vec::new();
        let mut parameters: Option<Node<'tree>> = None;
        let mut return_type: Option<Node<'tree>> = None;
        let mut precondition: Option<Node<'tree>> = None;
        let mut postcondition: Option<Node<'tree>> = None;
        let mut notes: Option<Node<'tree>> = None;
        let mut whole_feature: Option<Node<'tree>> = None;
        let mut attribute_or_routine: Option<Node<'tree>> = None;
        let mut body: Option<Node<'tree>> = None;

        let captures = match self.captures().next() {
            Some((mtc, _mtc_index)) => mtc.captures,
            None => return None,
        };

        for cap in captures {
            match cap.index {
                i if i == index_feature_name => names.push(cap.node),

                i if i == index_parameters => {
                    debug_assert!(parameters.is_none(), "There is maximum one parameter node.");
                    parameters = Some(cap.node)
                }
                i if i == index_return_type => {
                    debug_assert!(
                        return_type.is_none(),
                        "There is maximum one return type node."
                    );
                    return_type = Some(cap.node)
                }
                i if i == index_notes => {
                    debug_assert!(notes.is_none(), "There is maximum one notes node.");
                    notes = Some(cap.node)
                }
                i if i == index_precondition => {
                    debug_assert!(
                        precondition.is_none(),
                        "There is maximum one precondition node."
                    );
                    precondition = Some(cap.node)
                }
                i if i == index_postcondition => {
                    debug_assert!(
                        postcondition.is_none(),
                        "There is maximum one postcondition node."
                    );
                    postcondition = Some(cap.node)
                }
                i if i == index_attribute_or_routine => {
                    debug_assert!(
                        attribute_or_routine.is_none(),
                        "There is maximum one attribute_or_routine node."
                    );
                    attribute_or_routine = Some(cap.node)
                }
                i if i == index_body => {
                    debug_assert!(
                        body.is_none(),
                        "There is maximum one attribute_or_routine node."
                    );
                    body = Some(cap.node)
                }
                i if i == index_feature_declaration => {
                    debug_assert!(
                        whole_feature.is_none(),
                        "There is maximum one feature_declaration node."
                    );
                    whole_feature = Some(cap.node)
                }
                _ => {
                    unreachable!(
                        "The index of the capture must be handled by case.\nCapture kind:{}",
                        cap.node.kind()
                    )
                }
            }
        }

        Some(FeatureNodes {
            names,
            parameters,
            return_type,
            precondition,
            postcondition,
            notes,
            whole_feature: whole_feature?,
            body,
            attribute_or_routine,
        })
    }
}

impl<'source, 'tree> FeatureTree<'source, 'tree> for TreeTraversal<'source, 'tree> {
    fn goto_feature_tree(&mut self, feature_declaration_node: Node<'tree>) {
        debug_assert!(
            feature_declaration_node.kind() == "feature_declaration"
                || feature_declaration_node.kind() == "source_file"
        );
        self.set_node_and_query(feature_declaration_node, &FEATURE_QUERY);
    }

    fn feature_names(&mut self, feature_nodes: &FeatureNodes) -> Result<Vec<String>> {
        feature_nodes
            .names
            .iter()
            .map(|name_node| self.node_content(*name_node).map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()
    }

    fn feature_parameters(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<FeatureParameters> {
        let initial_node = self.current_node();

        match feature_nodes.parameters {
            Some(parameters_node) => {
                self.goto_parameter_tree(parameters_node);
                let parameters = self.parameters()?;
                self.goto_feature_tree(initial_node);
                Ok(parameters)
            }
            None => Ok(FeatureParameters::default()),
        }
    }

    fn feature_return_type(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<EiffelType>> {
        let initial_node = self.current_node();

        let return_type = feature_nodes
            .return_type
            .map(|type_node| {
                self.goto_eiffel_type_tree(type_node);
                let return_type = self.eiffel_type();
                self.goto_feature_tree(initial_node);
                return_type
            })
            .transpose();

        return_type
    }

    fn feature_precondition(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<Block<Precondition>>> {
        let initial_node = self.current_node();

        let notes_node = feature_nodes.notes;

        feature_nodes.precondition.map_or_else(
            || -> Result<Option<Block<Precondition>>> {
                Ok(feature_nodes
                    .attribute_or_routine
                    .map(|aor_node| aor_node.range())
                    .map(|range| {
                        let point_for_collapsed_block = match notes_node {
                            Some(note_node) => note_node.range().end_point,
                            None => range.start_point,
                        };
                        Block::new_empty(point_for_collapsed_block.into())
                    }))
            },
            |precondition_node| -> Result<Option<Block<Precondition>>> {
                self.goto_contract_tree(precondition_node);
                let clauses = self.clauses()?;
                let precondition = Precondition(clauses);
                let preconditions_block = Ok(Some(Block {
                    item: precondition,
                    range: precondition_node.range().into(),
                }));
                self.goto_feature_tree(initial_node);
                preconditions_block
            },
        )
    }

    fn feature_postcondition(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<Block<Postcondition>>> {
        let initial_node = self.current_node();

        feature_nodes.postcondition.map_or_else(
            || -> Result<Option<Block<_>>> {
                Ok(feature_nodes
                    .attribute_or_routine
                    .map(|aor_node| aor_node.range())
                    .map(|range| {
                        let mut point_of_collapsed_block: Point = range.end_point.into();

                        // Compensates the word `end`.
                        point_of_collapsed_block.shift_left(3);
                        Block::new_empty(point_of_collapsed_block)
                    }))
            },
            |postcondition_node| -> Result<Option<Block<_>>> {
                self.goto_contract_tree(postcondition_node);
                let clauses = self.clauses()?;
                let postcondition = Postcondition(clauses);
                let postconditions_block = Ok(Some(Block {
                    item: postcondition,
                    range: postcondition_node.range().into(),
                }));
                self.goto_feature_tree(initial_node);
                postconditions_block
            },
        )
    }

    fn feature_notes(
        &mut self,
        feature_nodes: &FeatureNodes<'tree>,
    ) -> Result<Option<FeatureNotes>> {
        let initial_node = self.current_node();

        feature_nodes
            .notes
            .map(|note_node| -> Result<_> {
                self.goto_notes_tree(note_node);
                let notes = self.notes();
                self.goto_feature_tree(initial_node);
                notes
            })
            .transpose()
    }

    fn feature(&mut self) -> Result<impl IntoIterator<Item = Feature>> {
        let feature_nodes = self
            .feature_nodes()
            .with_context(|| "Fails to parse feature")?;
        let names = self.feature_names(&feature_nodes)?;
        let parameters = self.feature_parameters(&feature_nodes)?;
        let return_type = self.feature_return_type(&feature_nodes)?;
        let notes = self.feature_notes(&feature_nodes)?;
        let range = feature_nodes.feature_range();
        let body_range = feature_nodes.feature_body_range();
        let preconditions = self.feature_precondition(&feature_nodes)?;
        let postconditions = self.feature_postcondition(&feature_nodes)?;

        Ok(names.into_iter().map(move |name| {
            Feature::new(
                name,
                parameters.clone(),
                return_type.clone(),
                notes.clone(),
                FeatureVisibility::Private,
                range.clone(),
                body_range.clone(),
                preconditions.clone(),
                postconditions.clone(),
            )
        }))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::parser::util::TreeTraversal;

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
        pub fn mock_features_clause<'tmp_src: 'source + 'tree>(
            parsed_file: &'tmp_src ParsedSource<'source>,
        ) -> anyhow::Result<Self> {
            let mut tree_traversal = parsed_file.class_tree_traversal()?;
            let nodes: ClassDeclarationNodes<'tree> = (&mut tree_traversal).try_into()?;
            let mut features = nodes.feature_clause_nodes;
            let first_feature_clause = features.pop().with_context(
                || "fails to get a feature to create the mock feature tree traversal.",
            )?;
            tree_traversal.set_node_and_query(
                first_feature_clause,
                &FEATURE_CLAUSE_QUERY,
            );
            Ok(tree_traversal)
        }

        pub fn mock_feature<'tmp_src: 'source + 'tree>(
            parsed_file: &'tmp_src ParsedSource<'source>,
        ) -> Self {
            let mut tree_traversal = parsed_file.class_tree_traversal().expect(
                "Should get tree traversal (implementing class traversal) from parsed file",
            );
            let nodes: ClassDeclarationNodes<'tree> = (&mut tree_traversal)
                .try_into()
                .expect("Should get class declaration nodes.");
            let mut features = nodes.feature_clause_nodes;
            let first_feature_clause = features.pop().expect("Should get first feature clause");
            tree_traversal.goto_feature_clause_tree(first_feature_clause);
            let first_feature = tree_traversal
                .nodes_captures("feature")
                .expect("Should find capture name `feature`")
                .pop()
                .expect("Should find first node `feature` in features clause.");
            tree_traversal.goto_feature_tree(first_feature);
            tree_traversal
        }
    }

    pub fn extracted_features(parsed_source: &ParsedSource) -> anyhow::Result<Vec<Feature>> {
        let mut feature_tree = TreeTraversal::mock_features_clause(&parsed_source)?;
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
            .notes()
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

    #[test]
    fn feature_nodes() {
        let mut parser = Parser::new();
        let parsed_source = parser
            .parse(CONTRACT_FEATURE_CLASS_SOURCE)
            .expect("Should parse `CONTRACT_FEATURE_CLASS_SOURCE`");

        let mut tree_traversal = TreeTraversal::mock_feature(&parsed_source);

        tree_traversal.feature_nodes();
    }
}
