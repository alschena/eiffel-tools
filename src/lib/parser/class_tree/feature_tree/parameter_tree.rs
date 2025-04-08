use anyhow::anyhow;
use anyhow::Context;

use super::FeatureParameters;
use crate::lib::parser::class_tree::eiffel_type::EiffelTypeTree;
use crate::lib::parser::util;
use crate::lib::parser::Node;
use crate::lib::parser::Query;

pub trait ParameterTree<'source, 'tree>: EntityDeclarationGroupTree<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
                (formal_arguments
                    (entity_declaration_group)* @entity_declaration_group
                )
            "#,
        )
    }

    fn goto_parameter_tree(&mut self, formal_arguments: Node<'tree>) {
        assert_eq!(formal_arguments.kind(), "formal_arguments");
        self.set_node_and_query(formal_arguments, <Self as ParameterTree>::query());
    }

    fn parameters(&mut self) -> Result<FeatureParameters, Self::Error> {
        self.nodes_captures("entity_declaration_group")?
            .iter()
            .map(|group_node| {
                self.goto_entity_declaration_group_tree(*group_node);
                self.entity_declaration_group_parameters()
            })
            .fold(Ok(FeatureParameters::default()), |acc, parameters| {
                let FeatureParameters {
                    mut names,
                    mut types,
                } = acc?;
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

pub trait EntityDeclarationGroupTree<'source, 'tree>: EiffelTypeTree<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
                (entity_declaration_group
                                    (identifier) @parameter_name
                                    ("," (identifier) @parameter_name)*
                                    type: (_) @parameter_type)
            "#,
        )
    }

    fn goto_entity_declaration_group_tree(&mut self, entity_declaration_group: Node<'tree>) {
        assert_eq!(entity_declaration_group.kind(), "entity_declaration_group");

        self.set_node_and_query(
            entity_declaration_group,
            <Self as EntityDeclarationGroupTree>::query(),
        );
    }

    fn entity_declaration_group_parameters(&mut self) -> Result<FeatureParameters, Self::Error> {
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

        if type_nodes.len() != 1 {
            return Err(anyhow!("fails to get exactly one node of type per entity declaration group. Type nodes: {type_nodes:#?}").into());
        }

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

impl<'source, 'tree, T: EiffelTypeTree<'source, 'tree>> EntityDeclarationGroupTree<'source, 'tree>
    for T
{
}
impl<'source, 'tree, T: EntityDeclarationGroupTree<'source, 'tree>> ParameterTree<'source, 'tree>
    for T
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::code_entities::prelude::*;
    use crate::lib::parser::class_tree::tests::FUNCTION_CLASS;
    use crate::lib::parser::Parser;
    use anyhow::Result;

    fn features(source: &str) -> Result<Vec<Feature>> {
        let mut parser = Parser::new();
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
