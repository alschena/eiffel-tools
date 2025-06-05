use super::Query;
use super::TreeTraversal;
use super::util;
use super::util::Traversal;
use crate::parser::FeatureName;
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

    fn top_level_calls_with_arguments(&mut self) -> Result<Vec<(FeatureName, Vec<String>)>>;
}

impl<'tree> ExpressionTree<'tree> for TreeTraversal<'_, 'tree> {
    fn top_level_identifiers(&mut self) -> Result<HashSet<&str>> {
        self.set_query(<Self as ExpressionTree>::query_top_level_identifiers());

        self.nodes_captures("id")?
            .into_iter()
            .map(|id_node| self.node_content(id_node))
            .collect::<Result<HashSet<_>>>()
    }

    fn top_level_calls_with_arguments(&mut self) -> Result<Vec<(FeatureName, Vec<String>)>> {
        let initial_node = self.current_node();

        self.set_query(<Self as ExpressionTree>::query_top_level_call_with_arguments());

        let mut top_level_calls = Vec::new();
        for call_node in self.nodes_captures("call")? {
            self.set_node(call_node);
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
                .filter_map(|argument_node| {
                    self.node_content(argument_node)
                        .inspect(|val| eprintln!("xxx: {val:#?}"))
                        .map(|arg| {
                            if arg.is_empty() {
                                None
                            } else {
                                Some(arg.to_string())
                            }
                        })
                        .inspect(|val| eprintln!("vvv: {val:#?}"))
                        .transpose()
                })
                .collect::<Result<Vec<_>>>()?;

            top_level_calls.push((id.into(), arguments));
        }
        self.set_node(initial_node);

        Ok(top_level_calls)
    }
}
