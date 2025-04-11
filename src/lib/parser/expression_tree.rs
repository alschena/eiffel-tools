use super::util;
use super::util::Traversal;
use super::Query;
use super::TreeTraversal;
use crate::lib::parser::Node;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashSet;

pub trait ExpressionTree<'tree> {
    fn query_top_level_identifiers() -> Query {
        util::query("(call (unqualified_call (identifier) @id) !target)")
    }

    fn top_level_identifiers(&mut self) -> Result<HashSet<&str>>;

    fn query_top_level_call_with_arguments() -> Query {
        util::query(
            r#"(call (unqualified_call (identifier) @id
                (actuals (expression) @argument
                    ("," (expression) @argument)*) !target)) @call"#,
        )
    }

    fn goto_call_node(&mut self, call_node: Node<'tree>);

    fn top_level_calls_with_arguments(&mut self) -> Result<Vec<(String, Vec<String>)>>;
}

impl<'source, 'tree> ExpressionTree<'tree> for TreeTraversal<'source, 'tree> {
    fn top_level_identifiers(&mut self) -> Result<HashSet<&str>> {
        self.nodes_captures("id")?
            .into_iter()
            .map(|id_node| self.node_content(id_node))
            .collect::<Result<HashSet<_>>>()
    }

    fn goto_call_node(&mut self, call_node: Node<'tree>) {
        assert_eq!(call_node.kind(), "call");
        self.set_node_and_query(
            call_node,
            <Self as ExpressionTree>::query_top_level_call_with_arguments(),
        );
    }

    fn top_level_calls_with_arguments(&mut self) -> Result<Vec<(String, Vec<String>)>> {
        let initial_node = self.current_node();

        let result = self
            .nodes_captures("call")?
            .into_iter()
            .map(|call_node| {
                self.goto_call_node(call_node);
                assert_eq!(self.current_node().kind(), "call");

                let id = self
                    .nodes_captures("id")?
                    .first()
                    .map(|id_node| {
                        self.node_content(*id_node)
                            .map(|content| content.to_string())
                    })
                    .with_context(|| "fails to find id node.")??;

                let arguments = self
                    .nodes_captures("argument")?
                    .into_iter()
                    .map(|argument_node| {
                        self.node_content(argument_node).map(|arg| arg.to_string())
                    })
                    .collect::<Result<Vec<_>>>()?;

                Ok((id, arguments))
            })
            .collect::<Result<Vec<_>>>();

        self.set_node(initial_node);
        result
    }
}
