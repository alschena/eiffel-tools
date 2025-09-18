use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;

use super::FeatureParameters;
use super::Traversal;
use super::TreeTraversal;
use crate::parser::Node;
use crate::parser::Query;
use crate::parser::class_tree::eiffel_type::EiffelTypeTree;
use crate::parser::util;

static PARAMETER_QUERY: LazyLock<Query> = LazyLock::new(|| {
    util::query(
        r#"
                (formal_arguments
                    (entity_declaration_group)* @entity_declaration_group
                )
            "#,
    )
});
static ENTITY_DECLARATION_GROUP: LazyLock<Query> = LazyLock::new(|| {
    util::query(
        r#"
                (entity_declaration_group
                                    (identifier) @parameter_name
                                    ("," (identifier) @parameter_name)*
                                    type: (_) @parameter_type)
            "#,
    )
});

pub trait ParameterTree<'source, 'tree> {
    fn goto_parameter_tree(&mut self, formal_arguments: Node<'tree>);
    fn parameters(&mut self) -> Result<FeatureParameters>;
}

pub trait EntityDeclarationGroupTree<'source, 'tree>: EiffelTypeTree<'source, 'tree> {
    fn goto_entity_declaration_group_tree(&mut self, entity_declaration_group: Node<'tree>);
    fn entity_declaration_group_parameters(&mut self) -> Result<FeatureParameters>;
}

impl<'source, 'tree> EntityDeclarationGroupTree<'source, 'tree> for TreeTraversal<'source, 'tree> {
    fn goto_entity_declaration_group_tree(&mut self, entity_declaration_group: Node<'tree>) {
        assert_eq!(entity_declaration_group.kind(), "entity_declaration_group");

        self.set_node_and_query(entity_declaration_group, &ENTITY_DECLARATION_GROUP);
    }

    fn entity_declaration_group_parameters(&mut self) -> Result<FeatureParameters> {
        assert_eq!(self.current_node().kind(), "entity_declaration_group");

        let names = self.nodes_captures("parameter_name")?;

        let names = names
            .into_iter()
            .map(|name_node| {
                self.node_content(name_node)
                    .map(|content| content.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let names_length = names.len();

        let type_nodes = self.nodes_captures("parameter_type")?;

        ensure!(
            type_nodes.len() == 1,
            "fails to get exactly one node of type per entity declaration group. Type nodes: {:#?}",
            type_nodes
        );

        let r#type = type_nodes
            .first()
            .with_context(|| "fails to get type of entitiy declaration group.")
            .map(|type_node| {
                self.goto_eiffel_type_tree(*type_node);
                self.eiffel_type()
            })??;

        Ok(FeatureParameters {
            names,
            types: vec![r#type; names_length],
        })
    }
}

impl<'source, 'tree, T> ParameterTree<'source, 'tree> for T
where
    T: EntityDeclarationGroupTree<'source, 'tree> + Traversal<'source, 'tree>,
{
    fn goto_parameter_tree(&mut self, formal_arguments: Node<'tree>) {
        assert_eq!(formal_arguments.kind(), "formal_arguments");
        self.set_node_and_query(formal_arguments, &PARAMETER_QUERY);
    }

    fn parameters(&mut self) -> Result<FeatureParameters> {
        self.nodes_captures("entity_declaration_group")?
            .iter()
            .map(|group_node| {
                self.goto_entity_declaration_group_tree(*group_node);
                self.entity_declaration_group_parameters()
            })
            .try_fold(FeatureParameters::default(), |acc, parameters| {
                let FeatureParameters {
                    mut names,
                    mut types,
                } = acc;
                let FeatureParameters {
                    names: mut new_names,
                    types: mut new_types,
                } = parameters?;
                names.append(&mut new_names);
                types.append(&mut new_types);

                Ok(FeatureParameters { names, types })
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::code_entities::prelude::*;
    use crate::parser::Parser;
    use crate::parser::class_tree::tests::FUNCTION_CLASS;
    use anyhow::Result;

    fn features(source: &str) -> Result<Vec<Feature>> {
        let mut parser = Parser::default();
        Ok(parser.class_from_source(source)?.features)
    }

    #[test]
    fn parse_parameters() -> anyhow::Result<()> {
        let feature = &(features(FUNCTION_CLASS)?)[0];

        assert_eq!(
            feature.parameters(),
            &FeatureParameters {
                names: vec!["y".to_string(), "z".to_string()],
                types: vec![
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    ),
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    )
                ]
            }
        );
        Ok(())
    }
}
