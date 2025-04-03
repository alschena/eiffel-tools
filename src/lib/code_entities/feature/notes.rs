use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use std::ops::Deref;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Notes(Vec<(String, Vec<String>)>);
impl Deref for Notes {
    type Target = Vec<(String, Vec<String>)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Parse for Notes {
    type Error = anyhow::Error;

    fn parse_through(
        node: &Node,
        cursor: &mut QueryCursor,
        src: &str,
    ) -> Result<Self, Self::Error> {
        let query = Self::query("(notes (note_entry)* @note_entry)");
        let query_note_entry = Self::query("(note_entry (tag) @tag value: (_)* @value)");

        let notes_entries: Vec<_> = cursor
            .matches(&query, *node, src.as_bytes())
            .filter_map_deref(|mat| capture_name_to_nodes("note_entry", &query, mat).next())
            .collect();

        let notes = notes_entries
            .iter()
            .filter_map(|note_entry_node| {
                let mut binding =
                    cursor.matches(&query_note_entry, *note_entry_node, src.as_bytes());
                let Some(mat) = binding.next() else {
                    return None;
                };
                let tag = capture_name_to_nodes("tag", &query_note_entry, mat)
                    .next()
                    .map_or_else(
                        || String::new(),
                        |ref tag| node_to_text(tag, src).to_string(),
                    );
                let values = capture_name_to_nodes("value", &query_note_entry, mat).fold(
                    Vec::new(),
                    |mut acc, ref value| {
                        acc.push(node_to_text(value, src).to_string());
                        acc
                    },
                );
                Some((tag, values))
            })
            .collect();
        Ok(Self(notes))
    }
}
