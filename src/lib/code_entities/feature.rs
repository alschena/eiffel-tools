use super::class::model::ModelExtended;
use super::prelude::*;
use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use async_lsp::lsp_types;
use contract::RoutineSpecification;
use contract::{Block, Postcondition, Precondition};
use std::fmt::Display;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tracing::warn;
use tree_sitter::{Node, QueryCursor};

mod notes;
use notes::Notes;

mod eiffel_type;
pub use eiffel_type::EiffelType;

mod parameters;
pub use parameters::Parameters;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
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
    pub fn is_feature_around_point(&self, point: Point) -> bool {
        point >= self.range().start && point <= self.range().end
    }
    pub fn feature_around_point<'feature>(
        mut features: impl Iterator<Item = &'feature Feature>,
        point: Point,
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
    pub fn routine_specification(&self) -> RoutineSpecification {
        let postcondition = self.postconditions().cloned().unwrap_or_default();
        let precondition = self.preconditions().cloned().unwrap_or_default();
        RoutineSpecification {
            precondition,
            postcondition,
        }
    }
    pub fn has_precondition(&self) -> bool {
        self.preconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn has_postcondition(&self) -> bool {
        self.postconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn point_end_preconditions(&self) -> Option<Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().end),
            None => return None,
        }
    }
    pub fn point_start_preconditions(&self) -> Option<Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().start),
            None => return None,
        }
    }
    pub fn point_end_postconditions(&self) -> Option<Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().end),
            None => None,
        }
    }
    pub fn point_start_postconditions(&self) -> Option<Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().start),
            None => None,
        }
    }
    pub fn supports_precondition_block(&self) -> bool {
        self.preconditions.is_some()
    }
    pub fn supports_postcondition_block(&self) -> bool {
        self.postconditions.is_some()
    }
    pub async fn src_unchecked<'src>(&self, path: &Path) -> anyhow::Result<String> {
        let range = self.range();
        let start_column = range.start.column;
        let start_row = range.start.row;
        let end_column = range.end.column;
        let end_row = range.end.row;

        let file_source = String::from_utf8(tokio::fs::read(&path).await?)?;
        let feature = file_source
            .lines()
            .skip(start_row)
            .enumerate()
            .map_while(|(linenum, line)| match linenum {
                0 => Some(&line[start_column..]),
                n if n < end_row - start_row => Some(line),
                n if n == end_row - start_row => Some(&line[..end_column]),
                _ => None,
            })
            .fold(String::new(), |mut acc, line| {
                acc.push_str(line);
                acc.push('\n');
                acc
            });
        Ok(feature)
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
    fn parse_through(node: &Node, cursor: &mut QueryCursor, src: &str) -> anyhow::Result<Self> {
        debug_assert!(node.kind() == "feature_declaration" || node.parent().is_none());

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
                    Parameters::parse_through(parameter_node, &mut cursor, src).ok()
                })
                .next()
                .unwrap_or_default();

            let return_type = capture_name_to_nodes("return_type", &query, mat)
                .next()
                .map(|ref return_type_node| {
                    EiffelType::parse_through(return_type_node, &mut cursor, src).ok()
                })
                .flatten();

            let notes = capture_name_to_nodes("notes", &query, mat)
                .next()
                .map(|note_node| {
                    Notes::parse_through(&note_node, &mut cursor, src)
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
                    |node| Block::<Precondition>::parse_through(&node, &mut cursor, src).ok(),
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
                    |node| Block::<Postcondition>::parse_through(&node, &mut cursor, src).ok(),
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
    use super::parameters::tests::integer_parameter;
    use super::parameters::tests::new_integer_parameter;
    use super::*;

    #[test]
    fn parse_feature_with_precondition() -> anyhow::Result<()> {
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
        let class = Class::parse(src)?;
        eprintln!("{class:?}");
        let feature = class.features().first().expect("first features is `x`");

        assert_eq!(feature.name(), "x");
        assert!(feature.preconditions().is_some());
        assert!(feature.preconditions().unwrap().first().is_some());

        let predicate = &feature.preconditions().unwrap().first().unwrap().predicate;
        assert_eq!(predicate.as_str(), "True");
        Ok(())
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

        let notes = Notes::parse_through(&node, &mut binding, &src).expect("Parse notes");
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

        let feature = Feature::parse_through(&node, &mut binding, &src).expect("Parse feature");
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
        let feature =
            Feature::parse_through(&node, &mut binding, src).expect("fails to parse feature.");

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
        assert!(eiffeltype
            .class_name()
            .is_ok_and(|name| name == *"MML_SEQUENCE"));
    }
}
