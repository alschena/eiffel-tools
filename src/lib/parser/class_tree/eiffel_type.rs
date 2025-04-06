use anyhow::Context;

use crate::lib::parser::util;
use crate::lib::parser::util::Traversal;
use crate::lib::parser::EiffelType;
use crate::lib::parser::Node;
use crate::lib::parser::Query;

pub trait EiffelTypeTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
            [
                (class_type (class_name) @class_name)  @class_type
                (tuple_type) @tuple_type
                (anchored) @anchored_type
            ] @eiffel_type
            "#,
        )
    }

    fn goto_eiffel_type_tree(&mut self, node: Node<'tree>) {
        assert!(
            node.kind() == "class_type" || node.kind() == "tuple_type" || node.kind() == "anchored"
        );
        self.set_node_and_query(node, <Self as EiffelTypeTree>::query());
    }

    fn eiffel_type(&mut self) -> Result<EiffelType, Self::Error> {
        match self.current_node().kind() {
            "class_type" => {
                let mut captures = self.nodes_captures("class_name")?;
                let outer_most_class_name_node = captures.pop().with_context(||"fails to get class_name of class_type.")?;
                Ok(EiffelType::ClassType(
                                self.node_content(self.current_node())?.to_string(),
                                self.node_content(outer_most_class_name_node)?.to_string(),
                    
                ))
            },
            "tuple_type" => {Ok(EiffelType::TupleType(self.node_content(self.current_node())?.to_string()))},
            "anchored" => {Ok(EiffelType::Anchored(self.node_content(self.current_node())?.to_string()))},
            _ => unreachable!("`EiffelTypeTree::eiffel_type` must be called from either `class_type`, `tuple_type` or `anchored` ")
        }
    }
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> EiffelTypeTree<'source, 'tree> for T {}
