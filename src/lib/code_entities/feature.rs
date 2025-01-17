use super::prelude::*;
use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use async_lsp::lsp_types;
use contract::{Block, Postcondition, Precondition};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tracing::warn;
use tree_sitter::{Node, Query, QueryCursor};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Notes(BTreeMap<String, String>);
impl Deref for Notes {
    type Target = BTreeMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Notes {
    pub(super) fn query() -> Query {
        Query::new(&tree_sitter_eiffel::LANGUAGE.into(), "(notes) @notes")
            .expect("Query tag for each note entry.")
    }
}

impl Parse for Notes {
    type Error = anyhow::Error;

    fn parse(node: &tree_sitter::Node, src: &str) -> Result<Self, Self::Error> {
        let mut cursor = QueryCursor::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();

        let query_note_entry = Query::new(&lang, "(note_entry) @note_entry")
            .expect("Query for `node_entry` nodes must succeed.");
        let mut matches_note_entries =
            cursor.matches(&query_note_entry, node.clone(), src.as_bytes());

        let mut note_entries = BTreeMap::new();
        while let Some(match_note_entry) = matches_note_entries.next() {
            for capture_note_entry in match_note_entry.captures {
                let mut query_cursor = QueryCursor::new();
                let node = capture_note_entry.node;

                let query_tag =
                    Query::new(&lang, "(tag) @tag").expect("Query tag for each note entry.");
                let query_value = Query::new(&lang, "value: (_) @value")
                    .expect("Query value for each note entry.");

                let mut tag_matches =
                    query_cursor.matches(&query_tag, node.clone(), src.as_bytes());
                let Some(tag_match) = tag_matches.next() else {
                    warn!("There must be a type for each entity declaration match.");
                    break;
                };
                let tag = src[tag_match.captures[0].node.byte_range()].into();

                let mut value_matches =
                    query_cursor.matches(&query_value, node.clone(), src.as_bytes());
                let Some(value_match) = value_matches.next() else {
                    warn!("There must be a type for each entity declaration match.");
                    break;
                };
                let value = src[value_match.captures[0].node.byte_range()].into();

                note_entries.insert(tag, value);
            }
        }

        Ok(Self(note_entries))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EiffelType {
    /// The first string is the whole string.
    /// The second string is the class name.
    ClassType(String, String),
    TupleType(String),
    Anchored(String),
}
impl Display for EiffelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            EiffelType::ClassType(s, _) => s,
            EiffelType::TupleType(s) => s,
            EiffelType::Anchored(s) => s,
        };
        write!(f, "{text}")
    }
}
impl EiffelType {
    fn class_name(&self) -> Result<&str, &str> {
        match self {
            EiffelType::ClassType(_, s) => Ok(s),
            EiffelType::TupleType(_) => Err("tuple type"),
            EiffelType::Anchored(_) => Err("anchored type"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Parameters(Vec<(String, EiffelType)>);
impl Deref for Parameters {
    type Target = Vec<(String, EiffelType)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Parameters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Parameters {
    fn add_parameter(&mut self, id: String, eiffel_type: EiffelType) {
        self.push((id, eiffel_type));
    }
    fn is_empty(&self) -> bool {
        self.deref().is_empty()
    }
}
impl Parse for Parameters {
    type Error = anyhow::Error;

    fn parse(node: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "formal_arguments");

        let mut cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();

        let parameter_query = Query::new(
            lang,
            r#"(entity_declaration_group
                (identifier) @name
                ("," (identifier) @name)*
                [
                    (class_type (class_name) @classname) @classtype
                    (tuple_type) @tupletype
                    (anchored) @anchoredtype
                ])"#,
        )
        .expect("fails to query parameter.");
        let mut parameters_matches = cursor.matches(&parameter_query, node.clone(), src.as_bytes());

        let mut parameters = Parameters(Vec::new());

        while let Some(mat) = parameters_matches.next() {
            let name_to_nodes = |name: &str| capture_name_to_nodes(name, &parameter_query, mat);
            let node_to_text = |node: Node<'_>| node_to_text(&node, &src);

            let names = name_to_nodes("name").map(|node| node_to_text(node).to_string());

            let class_type = name_to_nodes("classtype").next().map(|node| {
                EiffelType::ClassType(
                    node_to_text(node).to_string(),
                    node_to_text(
                        name_to_nodes("classname")
                            .next()
                            .expect("class type has class name."),
                    )
                    .to_string(),
                )
            });

            let tuple_type = name_to_nodes("tupletype")
                .next()
                .map(|node| EiffelType::TupleType(node_to_text(node).to_string()));

            let anchored_type = name_to_nodes("anchoredtype")
                .next()
                .map(|node| EiffelType::Anchored(node_to_text(node).to_string()));

            let eiffeltype = class_type
                .or(tuple_type.or(anchored_type))
                .expect("type is found.");

            names.for_each(|name| parameters.add_parameter(name, eiffeltype.clone()));
        }
        Ok(parameters)
    }
}
impl Display for Parameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.0.iter().fold(
            String::new(),
            |mut acc, (parameter_name, parameter_type)| {
                acc.push_str(parameter_name.as_str());
                acc.push_str(": ");
                acc.push_str(format!("{parameter_type}").as_str());
                acc
            },
        );
        write!(f, "{text}")?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Feature {
    pub(super) name: String,
    parameters: Parameters,
    return_type: String,
    notes: Option<Notes>,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    preconditions: Option<Block<Precondition>>,
    postconditions: Option<Block<Postcondition>>,
}
impl Feature {
    pub fn clone_rename(&self, name: String) -> Feature {
        let mut f = self.clone();
        f.name = name;
        f
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }
    pub fn number_parameters(&self) -> usize {
        self.parameters().len()
    }
    fn return_type(&self) -> &str {
        &self.return_type
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> Option<&Precondition> {
        self.preconditions.as_ref().map(|b| b.item())
    }
    pub fn postconditions(&self) -> Option<&Postcondition> {
        self.postconditions.as_ref().map(|b| b.item())
    }
    pub fn has_precondition(&self) -> bool {
        self.preconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn has_postcondition(&self) -> bool {
        self.postconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn point_end_preconditions(&self) -> Option<&Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().end()),
            None => return None,
        }
    }
    pub fn point_start_preconditions(&self) -> Option<&Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().start()),
            None => return None,
        }
    }
    pub fn point_end_postconditions(&self) -> Option<&Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().end()),
            None => None,
        }
    }
    pub fn point_start_postconditions(&self) -> Option<&Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().start()),
            None => None,
        }
    }
    pub fn supports_precondition_block(&self) -> bool {
        self.preconditions.is_some()
    }
    pub fn supports_postcondition_block(&self) -> bool {
        self.postconditions.is_some()
    }
}
impl Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name();
        let parenthesized_parameters = if self.parameters().is_empty() {
            String::new()
        } else {
            format!("({})", self.parameters())
        };
        let return_type = self.return_type();
        write!(f, "{name}{parenthesized_parameters}: {return_type}")
    }
}
impl Indent for Feature {
    const INDENTATION_LEVEL: usize = 1;
}
impl Parse for Feature {
    type Error = anyhow::Error;
    #[instrument(skip_all)]
    fn parse(node: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert!(node.kind() == "feature_declaration");

        let mut query_cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();

        let name_query = Query::new(lang, r#"(extended_feature_name) @name"#)
            .expect("Query for `extended_feature_name` must succeed.");
        let mut name_captures = query_cursor.captures(&name_query, node.clone(), src.as_bytes());
        let name = src[name_captures.next().expect("Should have name").0.captures[0]
            .node
            .byte_range()]
        .into();

        let parameters_query = Query::new(lang, "(formal_arguments) @parameters")
            .expect("Query for `formal_arguments` of the feature must succeed.");
        let parameters = query_cursor
            .captures(&parameters_query, node.clone(), src.as_bytes())
            .next()
            .map_or_else(
                || Ok(Parameters(Vec::new())),
                |formal_arguments| Parameters::parse(&formal_arguments.0.captures[0].node, src),
            )?;

        let return_type_query = Query::new(lang, "(feature_declaration type: (_) @return_type)")
            .expect("Query for the return type of the feature must succeed.");
        let return_type = query_cursor
            .captures(&return_type_query, node.clone(), src.as_bytes())
            .next()
            .map_or_else(
                || String::new(),
                |return_type| src[return_type.0.captures[0].node.byte_range()].into(),
            );

        let attribute_or_routine_captures_query =
            Query::new(lang, "(attribute_or_routine) @x").unwrap();
        let mut attribute_or_routine_captures = query_cursor.captures(
            &attribute_or_routine_captures_query,
            node.clone(),
            src.as_bytes(),
        );
        let aor = attribute_or_routine_captures.next();

        let notes;
        let preconditions;
        let postconditions;
        match aor {
            Some(aor) => {
                preconditions = Some(Block::<Precondition>::parse(&aor.0.captures[0].node, src)?);
                postconditions = Some(Block::<Postcondition>::parse(&aor.0.captures[0].node, src)?);
                notes = Some(Notes::parse(&aor.0.captures[0].node, src)?);
            }
            None => {
                preconditions = None;
                postconditions = None;
                notes = None;
            }
        }

        Ok(Feature {
            name,
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
            parameters,
            return_type,
            notes,
            preconditions,
            postconditions,
        })
    }
}
impl TryFrom<&Feature> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Feature) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let range = value.range().clone().try_into()?;
        Ok(lsp_types::DocumentSymbol {
            name,
            detail: None,
            kind: lsp_types::SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_feature_with_precondition() {
        let src = r#"
class A feature
  x
    require
      True
    do
    end

  y
    require else
    do
    end
end"#;
        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(&lang, "(feature_declaration) @feature").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;

        let feature = Feature::parse(&node, &src).expect("Parse feature");
        assert_eq!(feature.name(), "x");
        let predicate = feature
            .preconditions()
            .clone()
            .expect("extracted preconditions")
            .first()
            .expect("non empty precondition")
            .predicate
            .clone();
        assert_eq!(predicate.as_str(), "True")
    }

    #[test]
    fn parse_notes() {
        let src = r#"
class A feature
  x
    note
        entry_tag: entry_value
    do
    end
end
        "#;
        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(&lang, "(attribute_or_routine) @aor").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;

        let notes = Notes::parse(&node, &src).expect("Parse notes");
        let Some((tag, value)) = notes.iter().next() else {
            panic!("no note entries were found.")
        };
        assert_eq!(tag, "entry_tag");
        assert_eq!(value, "entry_value");
    }

    #[test]
    fn parse_notes_of_feature() {
        let src = r#"
class A feature
  x
    note
        entry_tag: entry_value
    do
    end
end
        "#;

        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(&lang, "(feature_declaration) @feature").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;

        let feature = Feature::parse(&node, &src).expect("Parse feature");
        let Some(feature_notes) = feature.notes else {
            panic!("feature notes have not been parsed.")
        };
        let Some((tag, value)) = feature_notes.iter().next() else {
            panic!("no note entries were found.")
        };
        assert_eq!(tag, "entry_tag");
        assert_eq!(value, "entry_value");
    }

    #[test]
    fn parse_parameters() {
        // Example feature
        let src = r#"
class A feature
  x (y, z: MML_SEQUENCE [INTEGER]): MML_SEQUENCE [INTEGER]
    do
    end
end
        "#;
        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();
        let query = ::tree_sitter::Query::new(&lang, "(feature_declaration) @feature").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;
        let feature = Feature::parse(&node, src).expect("fails to parse feature.");

        assert_eq!(
            feature.parameters(),
            &Parameters(vec![
                (
                    "y".to_string(),
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    )
                ),
                (
                    "z".to_string(),
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    )
                )
            ])
        );
    }

    #[test]
    fn parse_return_type() {
        // Example feature
        let src = r#"
class A feature
  x (y, z: MML_SEQUENCE [INTEGER]): MML_SEQUENCE [INTEGER]
    do
    end
end
        "#;
        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();
        let query = ::tree_sitter::Query::new(&lang, "(feature_declaration) @feature").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;
        let feature = Feature::parse(&node, src).expect("fails to parse feature.");

        assert_eq!(feature.return_type(), "MML_SEQUENCE [INTEGER]".to_string());
    }

    #[test]
    fn eiffel_type_class_name() {
        let eiffeltype = EiffelType::ClassType(
            "MML_SEQUENCE [INTEGER]".to_string(),
            "MML_SEQUENCE".to_string(),
        );
        assert_eq!(eiffeltype.class_name(), Ok("MML_SEQUENCE"));
    }
}
