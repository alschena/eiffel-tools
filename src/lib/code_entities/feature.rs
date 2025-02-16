use super::prelude::*;
use crate::lib::code_entities::class::model::ModelExtended;
use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use async_lsp::lsp_types;
use contract::{Block, Postcondition, Precondition};
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tracing::warn;
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

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
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
    pub fn class_name(&self) -> Result<&str, &str> {
        match self {
            EiffelType::ClassType(_, s) => Ok(s),
            EiffelType::TupleType(_) => Err("tuple type"),
            EiffelType::Anchored(_) => Err("anchored type"),
        }
    }
    pub fn class<'a, 'b: 'a>(
        &'a self,
        mut system_classes: impl Iterator<Item = &'b Class>,
    ) -> &'b Class {
        let class = system_classes
            .find(|&c| c.name() == self.class_name().unwrap_or_default())
            .unwrap_or_else(|| {
                panic!(
                    "parameters' class name: {}\tis in system.",
                    self.class_name().unwrap_or_default()
                )
            });
        class
    }
    pub fn is_terminal_for_model(&self) -> bool {
        match self.class_name() {
            Ok("BOOLEAN") => true,
            Ok("INTEGER") => true,
            Ok("REAL") => true,
            Ok("MML_SEQUENCE") => true,
            Ok("MML_BAG") => true,
            Ok("MML_SET") => true,
            Ok("MML_MAP") => true,
            Ok("MML_PAIR") => true,
            Ok("MML_RELATION") => true,
            Err("tuple type") => unimplemented!(),
            _ => false,
        }
    }
}

impl Parse for EiffelType {
    type Error = anyhow::Error;

    fn parse(node: &Node, query_cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        let eiffeltype = match node.kind() {
            "class_type" => {
                let query = Self::query("(class_name) @classname");
                let mut matches = query_cursor.matches(&query, *node, src.as_bytes());
                let mat = matches.next().expect("match for classname in classtype.");
                let classname_node = capture_name_to_nodes("classname", &query, mat)
                    .next()
                    .expect("capture for classname in classtype.");

                let classname = node_to_text(&classname_node, src).to_string();
                EiffelType::ClassType(node_to_text(&node, src).to_string(), classname)
            }
            "tuple_type" => EiffelType::TupleType(node_to_text(&node, src).to_string()),
            "anchored" => EiffelType::Anchored(node_to_text(&node, src).to_string()),
            _ => unreachable!(),
        };
        Ok(eiffeltype)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Parameters {
    names: Vec<String>,
    types: Vec<EiffelType>,
}
impl Parameters {
    pub fn names(&self) -> &Vec<String> {
        &self.names
    }
    pub fn types(&self) -> &Vec<EiffelType> {
        &self.types
    }
    fn add_parameter(&mut self, id: String, eiffel_type: EiffelType) {
        self.names.push(id);
        self.types.push(eiffel_type);
    }
    fn is_empty(&self) -> bool {
        self.names().is_empty() && self.types().is_empty()
    }
    pub fn full_extended_models<'s, 'system>(
        &'s self,
        system_classes: &'system [&Class],
    ) -> impl Iterator<Item = ModelExtended> + use<'s, 'system> {
        self.types().iter().map(|t| {
            t.class(system_classes.iter().copied())
                .full_extended_model(system_classes)
        })
    }
}
impl Parse for Parameters {
    type Error = anyhow::Error;

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "formal_arguments");

        let parameter_query = Self::query(
            r#"(entity_declaration_group
                (identifier) @name
                ("," (identifier) @name)*
                type: (_) @eiffeltype
                )"#,
        );
        let mut parameters_matches = cursor.matches(&parameter_query, node.clone(), src.as_bytes());

        let mut parameters = Parameters::default();

        while let Some(mat) = parameters_matches.next() {
            let name_to_nodes = |name: &str| capture_name_to_nodes(name, &parameter_query, mat);
            let node_to_text = |node: Node<'_>| node_to_text(&node, &src);

            let names = name_to_nodes("name").map(|node| node_to_text(node).to_string());

            let eiffeltype = EiffelType::parse(
                &name_to_nodes("eiffeltype")
                    .next()
                    .expect("captured eiffel type."),
                &mut QueryCursor::new(),
                src,
            )
            .expect("parse parameters.");

            names.for_each(|name| parameters.add_parameter(name, eiffeltype.clone()));
        }
        Ok(parameters)
    }
}
impl Display for Parameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let types = self.types();
        let names = self.names();
        let text = names.iter().zip(types.iter()).fold(
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
    return_type: Option<EiffelType>,
    notes: Option<Notes>,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    preconditions: Option<Block<Precondition>>,
    postconditions: Option<Block<Postcondition>>,
}
impl Feature {
    pub fn is_feature_around_point(&self, point: &Point) -> bool {
        point >= self.range().start() && point <= self.range().end()
    }
    pub fn feature_around_point<'feature>(
        mut features: impl Iterator<Item = &'feature Feature>,
        point: &Point,
    ) -> Option<&'feature Feature> {
        features.find(|f| f.is_feature_around_point(point))
    }
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
        let parameters = self.parameters();
        debug_assert_eq!(parameters.names().len(), parameters.types().len());
        parameters.names().len()
    }
    pub fn return_type(&self) -> Option<&EiffelType> {
        self.return_type.as_ref()
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
        let format_return_type = self.return_type().map_or_else(
            || String::new(),
            |ref return_type| format!(": {return_type}"),
        );
        write!(f, "{name}{parenthesized_parameters}{format_return_type}")
    }
}
impl Indent for Feature {
    const INDENTATION_LEVEL: usize = 1;
}
impl Parse for Feature {
    type Error = anyhow::Error;
    #[instrument(skip_all)]
    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> anyhow::Result<Self> {
        debug_assert!(node.kind() == "feature_declaration");

        let query = Self::query(
            r#"
            (feature_declaration (new_feature (extended_feature_name) @name)
            ("," (new_feature (extended_feature_name) @name))*
            (formal_arguments)? @parameters
            type: (_)? @return_type
            (attribute_or_routine
                (notes)? @notes
                (precondition)? @precondition
                (postcondition)? @postcondition)? @attribute_or_routine)
            "#,
        );

        let mut matches = cursor.matches(&query, *node, src.as_bytes());

        let mut feature: Option<Feature> = None;
        while let Some(mat) = matches.next() {
            let name = capture_name_to_nodes("name", &query, mat)
                .map(|ref name_node| node_to_text(name_node, src).to_string())
                .next()
                .expect("capture feature name.");

            let mut cursor = QueryCursor::new();

            let parameters = capture_name_to_nodes("parameters", &query, mat)
                .filter_map(|ref parameter_node| {
                    Parameters::parse(parameter_node, &mut cursor, src).ok()
                })
                .next()
                .unwrap_or_default();

            let return_type = capture_name_to_nodes("return_type", &query, mat)
                .next()
                .map(|ref return_type_node| {
                    EiffelType::parse(return_type_node, &mut cursor, src).ok()
                })
                .flatten();

            let notes = capture_name_to_nodes("notes", &query, mat)
                .next()
                .map(|note_node| {
                    Notes::parse(&note_node, &mut cursor, src)
                        .ok()
                        .map(|notes| (notes, note_node.range()))
                })
                .flatten();
            let (notes, notes_range) = match notes {
                Some((n, r)) => (Some(n), Some(r)),
                None => (None, None),
            };

            // If this node is captured, the contract blocks are allowed.
            let attribute_or_routine =
                capture_name_to_nodes("attribute_or_routine", &query, mat).next();
            let preconditions = capture_name_to_nodes("precondition", &query, mat)
                .next()
                .map_or_else(
                    || {
                        attribute_or_routine
                            .map(|aor| aor.range().start_point)
                            .map(|aor_point| notes_range.map_or_else(|| aor_point, |r| r.end_point))
                            .map(|point| Block::new_empty(point.into()))
                    },
                    |node| Block::<Precondition>::parse(&node, &mut cursor, src).ok(),
                );

            let postconditions = capture_name_to_nodes("postcondition", &query, mat)
                .next()
                .map_or_else(
                    || {
                        attribute_or_routine
                            .map(|aor| Point::from(aor.range().end_point))
                            .map(|mut point| {
                                // This compensates the keyword `end`
                                point.shift_left(3);
                                point
                            })
                            .map(|point| Block::new_empty(point))
                    },
                    |node| Block::<Postcondition>::parse(&node, &mut cursor, src).ok(),
                );

            feature = Some(Feature {
                name,
                visibility: FeatureVisibility::Private,
                range: node.range().into(),
                parameters,
                return_type,
                notes,
                preconditions,
                postconditions,
            });
        }

        Ok(feature.expect("parsed feature."))
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
        let class = Class::from_source(src);
        eprintln!("{class:?}");
        let feature = class.features().first().expect("first features is `x`");

        assert_eq!(feature.name(), "x");
        assert!(feature.preconditions().is_some());
        assert!(feature.preconditions().unwrap().first().is_some());

        let predicate = &feature.preconditions().unwrap().first().unwrap().predicate;
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

        let notes = Notes::parse(&node, &mut binding, &src).expect("Parse notes");
        let Some((tag, value)) = notes.iter().next() else {
            panic!("no note entries were found.")
        };
        assert_eq!(tag, "entry_tag");
        assert_eq!(value.first().unwrap(), "entry_value");
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

        let feature = Feature::parse(&node, &mut binding, &src).expect("Parse feature");
        let Some(feature_notes) = feature.notes else {
            panic!("feature notes have not been parsed.")
        };
        let Some((tag, value)) = feature_notes.iter().next() else {
            panic!("no note entries were found.")
        };
        assert_eq!(tag, "entry_tag");
        assert_eq!(value.first().unwrap(), "entry_value");
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
        let class = Class::from_source(src);
        let feature = class.features().first().expect("parsed feature.");

        assert_eq!(
            feature.parameters(),
            &Parameters {
                names: vec!["y".to_string(), "z".to_string()],
                types: vec![
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    ),
                    EiffelType::ClassType(
                        "MML_SEQUENCE [INTEGER]".to_string(),
                        "MML_SEQUENCE".to_string()
                    )
                ]
            }
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
        let feature = Feature::parse(&node, &mut binding, src).expect("fails to parse feature.");

        let return_type = feature.return_type().unwrap();
        assert_eq!(
            format!("{return_type}"),
            "MML_SEQUENCE [INTEGER]".to_string()
        );
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
