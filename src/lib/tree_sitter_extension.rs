use tree_sitter::Node;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::QueryMatch;

pub fn node_to_text<'a>(node: &Node<'_>, src: &'a str) -> &'a str {
    node.utf8_text(src.as_bytes()).expect("node has text.")
}

pub fn capture_name_to_nodes<'tree, 'cursor, 'querymatch>(
    capture_name: &str,
    query: &Query,
    query_match: &'querymatch QueryMatch<'cursor, 'tree>,
) -> impl Iterator<Item = Node<'tree>> + use<'cursor, 'tree, 'querymatch> {
    query_match.nodes_for_capture_index(
        query
            .capture_index_for_name(capture_name)
            .unwrap_or_else(|| panic!("capture name: {capture_name}")),
    )
}

pub trait Parse: Sized {
    type Error;
    fn query(sexp: &str) -> Query {
        Query::new(&tree_sitter_eiffel::LANGUAGE.into(), sexp)
            .unwrap_or_else(|e| panic!("query:\t{sexp}\n\thas error: {e}"))
    }
    fn parse(node: &Node, query_cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error>;
}
