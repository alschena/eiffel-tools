use crate::parser::*;
use anyhow::Result;
use anyhow::anyhow;
use tree_sitter::QueryCaptures;

pub fn query(sexp: &str) -> Query {
    Query::new(&tree_sitter_eiffel::LANGUAGE.into(), sexp)
        .unwrap_or_else(|e| panic!("query:\t{sexp}\n\thas error: {e}"))
}

pub fn is_inside<'tree>(inner: Node<'tree>, outer: Node<'tree>) -> bool {
    let outer_range = outer.range();
    let outer_start = outer_range.start_byte;
    let outer_end = outer_range.end_byte;

    let inner_range = inner.range();
    let inner_start = inner_range.start_byte;
    let inner_end = inner_range.end_byte;

    outer_start <= inner_start && inner_end <= outer_end
}

pub trait Traversal<'source, 'tree> {
    fn current_node(&self) -> Node<'tree>;
    fn node_content(&self, node: Node<'tree>) -> Result<&str>;
    fn nodes_captures(&mut self, capture_name: &str) -> Result<Vec<Node<'tree>>>;
    fn set_node_and_query(&mut self, node: Node<'tree>, query: &'static Query);
}

pub struct TreeTraversal<'source, 'tree> {
    source: &'source [u8],
    node: Node<'tree>,
    query: &'static Query,
    cursor: QueryCursor,
}

impl<'source, 'tree> Traversal<'source, 'tree> for TreeTraversal<'source, 'tree> {
    fn current_node(&self) -> Node<'tree> {
        self.node
    }

    fn node_content(&self, node: Node<'tree>) -> Result<&str> {
        node.utf8_text(self.source)
            .map_err(|e| anyhow!("fails to extract content from node: {node} with error: {e}"))
    }

    fn nodes_captures(&mut self, capture_name: &str) -> Result<Vec<Node<'tree>>> {
        let index = self
            .query
            .capture_index_for_name(capture_name)
            .with_context(|| format!("fails to find {capture_name} as a capture name."))?;

        let nodes = self
            .cursor
            .matches(self.query, self.node, self.source)
            .fold(Vec::new(), |mut acc, mtc| {
                acc.extend(mtc.nodes_for_capture_index(index));
                acc
            });
        Ok(nodes)
    }

    fn set_node_and_query(&mut self, node: Node<'tree>, query: &'static Query) {
        self.set_node(node);
        self.set_query(query);
    }
}

impl<'source, 'tree> TreeTraversal<'source, 'tree> {
    pub fn try_new(
        source: &'source [u8],
        node: Node<'tree>,
        query: &'static Query,
    ) -> Result<Self> {
        let cursor = QueryCursor::new();

        Ok(Self {
            source,
            node,
            query,
            cursor,
        })
    }

    pub(super) fn set_node(&mut self, node: Node<'tree>) {
        self.node = node
    }

    pub(super) fn set_query(&mut self, query: &'static Query) {
        self.query = query
    }

    pub(super) fn capture_index_of(&self, capture_name: &str) -> u32 {
        self.query
            .capture_index_for_name(capture_name)
            .unwrap_or_else(|| panic!("Should find the capture index of `{capture_name}`"))
    }

    pub(super) fn captures(&mut self) -> QueryCaptures<'_, 'tree, &[u8], &[u8]> {
        self.cursor.captures(self.query, self.node, self.source)
    }
}
