use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid contract clause of the eiffel programming language.")]
pub struct Clause {
    pub tag: Tag,
    pub predicate: Predicate,
}

impl Default for Clause {
    fn default() -> Self {
        Self {
            tag: <Tag as Default>::default(),
            predicate: <Predicate as Default>::default(),
        }
    }
}

impl Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.tag, &self.predicate) {
            (_, predicate) if predicate.as_str().is_empty() => {
                write!(f, "")
            }
            (tag, predicate) if tag.as_str().is_empty() => {
                write!(f, "({})\n", predicate)
            }
            (tag, predicate) => {
                write!(f, "{}: {}\n", tag, predicate)
            }
        }
    }
}

impl Clause {
    pub fn new(tag: Tag, predicate: Predicate) -> Clause {
        Clause { tag, predicate }
    }
    pub fn from_line(line: &str) -> Option<Clause> {
        line.rsplit_once(": ").map(|(tag_str, predicate_str)| {
            Clause::new(
                Tag::new(tag_str.trim()),
                Predicate::new(predicate_str.trim()),
            )
        })
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid tag clause for the Eiffel programming language.")]
pub struct Tag(String);

impl Tag {
    pub fn new<T: ToString>(text: T) -> Tag {
        Tag(text.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn trim_and_replace_space_with_underscore(&mut self) {
        self.0 = self.0.trim().replace(" ", "_");
    }
    pub fn update_to_lowercase(&mut self) {
        self.0 = self.0.to_lowercase();
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self(String::new())
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag(value)
    }
}

#[derive(Hash, Deserialize, Debug, PartialEq, Eq, Clone, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid boolean expression for the Eiffel programming language.")]
pub struct Predicate(String);

impl From<&str> for Predicate {
    fn from(value: &str) -> Self {
        Predicate(value.to_string())
    }
}

impl Predicate {
    pub fn new<T: ToString>(text: T) -> Predicate {
        Predicate(text.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parse(&self) -> Option<Tree> {
        let text: &str = self.as_str();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&lang)
            .expect("parser must load grammar.");
        parser.parse(text, None)
    }

    pub fn top_level_identifiers(&self) -> HashSet<&str> {
        let tree = self.parse().expect("fails to parse predicate.");
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        let text = self.as_str();

        let query_id = Query::new(&lang, "(call (unqualified_call (identifier) @id) !target)")
            .expect("Fails to construct query for top-level identifiers (names of unqualified features and targets) in predicate: {self}");

        let mut query_cursor = QueryCursor::new();

        let mut matches = query_cursor.matches(&query_id, tree.root_node(), text.as_bytes());

        let mut ids = HashSet::new();
        while let Some(mat) = matches.next() {
            for cap in mat.captures.iter() {
                let id = cap
                    .node
                    .utf8_text(text.as_bytes())
                    .expect("The capture must contain valid text.");
                ids.insert(id);
            }
        }
        ids
    }
}

impl Default for Predicate {
    fn default() -> Self {
        Self(String::from("True"))
    }
}

impl Display for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_identifiers() {
        let p = Predicate("x < y.z.w".to_string());
        let ids = p.top_level_identifiers();
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.len() == 2);
    }

    #[test]
    fn predicate_identifiers_unqualified_calls() {
        let p = Predicate("x (y) < y (l).z.w".to_string());
        let ids = p.top_level_identifiers();
        eprintln!("{ids:?}");
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.contains("l"));
        assert!(ids.len() == 3);
    }
}
