use crate::lib::parser::*;
use anyhow::anyhow;

pub fn query(sexp: &str) -> Query {
    Query::new(&tree_sitter_eiffel::LANGUAGE.into(), sexp)
        .unwrap_or_else(|e| panic!("query:\t{sexp}\n\thas error: {e}"))
}

pub trait Nodes<'source, 'tree> {
    type Error;
    fn current_node(&self) -> Node<'tree>;
    fn matches(&mut self) -> QueryMatches<'_, 'tree, &[u8], &[u8]>;
    fn node_content(&self, node: Node<'tree>) -> Result<&str, Self::Error>;
    fn nodes_captures(&mut self, capture_name: &str) -> Result<Vec<Node<'tree>>, Self::Error>;
    fn goto_node(&mut self, node: Node<'tree>);
    fn set_query(&mut self, query: Query);
}

pub struct TreeTraversal<'source, 'tree> {
    source: &'source [u8],
    node: Node<'tree>,
    query: Query,
    cursor: QueryCursor,
}

impl<'source, 'tree> TryFrom<&'tree ParsedSource<'source>> for TreeTraversal<'source, 'tree> {
    type Error = anyhow::Error;

    fn try_from(value: &'tree ParsedSource<'source>) -> anyhow::Result<Self> {
        let source = value.source;
        let node = value.tree.root_node();
        let query = <TreeTraversal as ClassTree>::query();
        Self::try_new(source, node, query)
    }
}

impl<'source, 'tree> Nodes<'source, 'tree> for TreeTraversal<'source, 'tree> {
    type Error = anyhow::Error;

    fn current_node(&self) -> Node<'tree> {
        self.node
    }

    fn matches(&mut self) -> QueryMatches<'_, 'tree, &[u8], &[u8]> {
        self.cursor.matches(&self.query, self.node, self.source)
    }

    fn node_content(&self, node: Node<'tree>) -> Result<&str, Self::Error> {
        node.utf8_text(self.source)
            .map_err(|e| anyhow!("fails to extract content from node: {node} with error: {e}"))
    }

    fn nodes_captures(&mut self, capture_name: &str) -> anyhow::Result<Vec<Node<'tree>>> {
        let index = self
            .query
            .capture_index_for_name(capture_name)
            .with_context(|| "fails to find `notes` as a capture name.")?;

        let nodes = self.matches().fold(Vec::new(), |mut acc, mtc| {
            acc.extend(mtc.nodes_for_capture_index(index));
            acc
        });
        Ok(nodes)
    }

    fn goto_node(&mut self, node: Node<'tree>) {
        self.node = node;
    }

    fn set_query(&mut self, query: Query) {
        self.query = query;
    }
}

impl<'source, 'tree> TreeTraversal<'source, 'tree> {
    pub fn try_new(source: &'source [u8], node: Node<'tree>, query: Query) -> anyhow::Result<Self> {
        let cursor = QueryCursor::new();

        Ok(Self {
            source,
            node,
            query,
            cursor,
        })
    }
}
