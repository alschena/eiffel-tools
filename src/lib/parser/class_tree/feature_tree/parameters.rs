use super::FeatureParameters;
use crate::lib::parser::class_tree::eiffel_type::EiffelTypeTree;
use crate::lib::parser::util;
use crate::lib::parser::Node;
use crate::lib::parser::Query;

pub trait ParameterTree<'source, 'tree>: EiffelTypeTree<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
                (formal_arguments
                    (entity_declaration_group
                                    (identifier) @parameter_name
                                    ("," (identifier) @parameter_name)*
                                    type: (_) @parameter_type)
                )
            "#,
        )
    }

    fn goto_parameter_tree(&mut self, formal_arguments: Node<'tree>) {
        assert_eq!(formal_arguments.kind(), "formal_arguments");
        self.set_node_and_query(formal_arguments, <Self as ParameterTree>::query());
    }

    fn parameters(&mut self) -> Result<FeatureParameters, Self::Error> {
        let names = self
            .nodes_captures("parameter_name")?
            .iter()
            .map(|&name_node| self.node_content(name_node).map(|name| name.to_string()))
            .collect::<Result<Vec<_>, Self::Error>>()?;
        let types = self
            .nodes_captures("parameter_type")?
            .iter()
            .map(|&type_node| -> Result<_, Self::Error> {
                self.goto_eiffel_type_tree(type_node);
                self.eiffel_type()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(FeatureParameters { names, types })
    }
}

impl<'source, 'tree, T: EiffelTypeTree<'source, 'tree>> ParameterTree<'source, 'tree> for T {}
