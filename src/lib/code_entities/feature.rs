use super::class::Class;
use super::contract::{Block, Postcondition, Precondition};
use super::prelude::*;
use crate::lib::tree_sitter_extension::{self, Node, Parse};
use ::tree_sitter::{Query, QueryCursor};
use anyhow::anyhow;
use async_lsp::lsp_types;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Feature {
    pub(super) name: String,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    pub(super) preconditions: Option<Block<Precondition>>,
    pub(super) postconditions: Option<Block<Postcondition>>,
}
impl Feature {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> &Option<Block<Precondition>> {
        &self.preconditions
    }
    pub fn is_precondition_block_present(&self) -> bool {
        match &self.preconditions {
            Some(Block { item, .. }) => match item {
                Some(_) => true,
                None => false,
            },
            None => false,
        }
    }
    pub fn is_postcondition_block_present(&self) -> bool {
        match &self.postconditions {
            Some(Block { item, .. }) => match item {
                Some(_) => true,
                None => false,
            },
            None => false,
        }
    }
    pub fn range_end_preconditions(&self) -> Option<Range> {
        let point: &Point = match &self.preconditions {
            Some(pre) => &pre.range().end,
            None => return None,
        };
        Some(Range {
            start: point.clone(),
            end: point.clone(),
        })
    }
    pub fn range_start_preconditions(&self) -> Option<Range> {
        let point: &Point = match &self.preconditions {
            Some(pre) => &pre.range().start,
            None => return None,
        };
        Some(Range {
            start: point.clone(),
            end: point.clone(),
        })
    }
    pub fn range_end_postconditions(&self) -> Option<Range> {
        let point: &Point = match &self.postconditions {
            Some(post) => &post.range().end,
            None => return None,
        };
        Some(Range {
            start: point.clone(),
            end: point.clone(),
        })
    }
    pub fn range_start_postconditions(&self) -> Option<Range> {
        let point: &Point = match &self.postconditions {
            Some(post) => &post.range().start,
            None => return None,
        };
        Some(Range {
            start: point.clone(),
            end: point.clone(),
        })
    }
}
impl Indent for Feature {
    const INDENTATION_LEVEL: u32 = 1;
}
impl Parse for Feature {
    type Error = anyhow::Error;
    #[instrument(skip_all)]
    fn parse(node: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert!(node.kind() == "feature_declaration");
        let mut binding = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = Query::new(lang, "(extended_feature_name) @name").unwrap();
        let mut name_captures = binding.captures(&query, node.clone(), src.as_bytes());
        let name = src[name_captures.next().expect("Should have name").0.captures[0]
            .node
            .byte_range()]
        .into();

        let query = Query::new(lang, "(attribute_or_routine) @x").unwrap();
        let mut attribute_or_routine_captures =
            binding.captures(&query, node.clone(), src.as_bytes());
        let aor = attribute_or_routine_captures.next();
        let preconditions = match aor {
            Some(x) => Some(Block::<Precondition>::parse(&x.0.captures[0].node, src)?),
            None => None,
        };
        let postconditions = match aor {
            Some(x) => Some(Block::<Postcondition>::parse(&x.0.captures[0].node, src)?),
            None => None,
        };

        Ok(Feature {
            name,
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
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
    use crate::lib::tree_sitter_extension::WidthFirstTraversal;

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
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = ::tree_sitter::Query::new(lang, "(feature_declaration) @name").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;

        let feature = Feature::parse(&node, &src).expect("Parse feature");
        assert_eq!(feature.name(), "x");
        let predicate = feature
            .preconditions()
            .as_ref()
            .expect("fails because feature cannot have a precondition block.")
            .item()
            .clone()
            .expect("extracted preconditions")
            .precondition
            .first()
            .expect("non empty precondition")
            .predicate
            .clone()
            .predicate;
        assert_eq!(predicate, "True".to_string())
    }
}
