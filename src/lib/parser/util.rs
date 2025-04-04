use crate::lib::parser::*;

pub fn query(sexp: &str) -> Query {
    Query::new(&tree_sitter_eiffel::LANGUAGE.into(), sexp)
        .unwrap_or_else(|e| panic!("query:\t{sexp}\n\thas error: {e}"))
}

pub trait Nodes {
    type Error;
    fn nodes(&mut self, capture_name: &str) -> Result<Vec<Node<'_>>, Self::Error>;
    fn matches(&mut self) -> QueryMatches<'_, '_, &[u8], &[u8]>;
}

pub struct TreeTraversal<'source, 'tree> {
    source: &'source [u8],
    node: Node<'tree>,
    query: Query,
    cursor: QueryCursor,
}

impl<'source, 'tree> Nodes for TreeTraversal<'source, 'tree> {
    type Error = anyhow::Error;

    fn nodes(&mut self, capture_name: &str) -> anyhow::Result<Vec<Node<'_>>> {
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

    fn matches(&mut self) -> QueryMatches<'_, '_, &[u8], &[u8]> {
        self.cursor.matches(&self.query, self.node, self.source)
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
