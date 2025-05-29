use crate::lib::parser::Node;
use anyhow::Context;
use anyhow::Result;

use crate::lib::parser::util;
use crate::lib::parser::util::is_inside;
use crate::lib::parser::util::Traversal;
use crate::lib::parser::Query;

use super::contract::Clause;

pub trait ContractTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"(assertion_clause (tag_mark (tag) @tag)? (expression) @expression)* @clause"#,
        )
    }

    fn goto_contract_tree(&mut self, contract_node: Node<'tree>);

    fn clauses(&mut self) -> Result<Vec<Clause>>;
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> ContractTree<'source, 'tree> for T {
    fn goto_contract_tree(&mut self, contract_node: Node<'tree>) {
        assert!(
            contract_node.kind() == "precondition"
                || contract_node.kind() == "postcondition"
                || contract_node.kind() == "invariant"
        );
        self.set_node_and_query(contract_node, <Self as ContractTree>::query());
    }

    fn clauses(&mut self) -> Result<Vec<Clause>> {
        let clauses = self.nodes_captures("clause")?;
        let tag = self.nodes_captures("tag")?;
        let expression = self.nodes_captures("expression")?;

        let clauses = clauses
            .into_iter()
            .map(|clause_node| -> Result<_> {
                let tag_node = tag
                    .iter()
                    .find(|&&tag_node| is_inside(tag_node, clause_node));
                let predicate_node = expression
                    .iter()
                    .find(|&&expression_node| is_inside(expression_node, clause_node))
                    .with_context(|| {
                        format!("fails to get expression of contract clause node: {clause_node}")
                    })?;

                let tag = tag_node
                    .map(|&tag_node| -> Result<_> {
                        let content = self.node_content(tag_node)?;
                        let owned_content = content.to_string();
                        Ok(owned_content.into())
                    })
                    .transpose()?
                    .unwrap_or_default();

                let predicate = self.node_content(*predicate_node)?.into();
                Ok(Clause { tag, predicate })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(clauses)
    }
}
