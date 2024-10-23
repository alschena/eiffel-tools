use super::class::Class;
use super::contract::Contract;
use super::prelude::*;
use crate::lib::tree_sitter::{self, Node, Parse};
use ::tree_sitter::{Query, QueryCursor};
use anyhow::anyhow;
use async_lsp::lsp_types;
use streaming_iterator::StreamingIterator;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Feature {
    pub(super) name: String,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    pub(super) preconditions: Option<Contract<Precondition>>,
    pub(super) postconditions: Option<Contract<Postcondition>>,
}
impl Feature {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> &Option<Contract<Precondition>> {
        &self.preconditions
    }
    pub fn is_precondition_block_present(&self) -> bool {
        match &self.preconditions {
            Some(Contract { item, .. }) => match item {
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
}
impl Indent for Feature {
    const INDENTATION_LEVEL: u32 = 1;
}
impl Parse for Feature {
    type Error = anyhow::Error;
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
            Some(x) => Some(Contract::parse(&x.0.captures[0].node, src)?),
            None => None,
        };

        Ok(Feature {
            name,
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
            preconditions,
            postconditions: None,
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
    use crate::lib::tree_sitter::WidthFirstTraversal;

    use super::*;

    #[test]
    fn extract_feature_with_precondition() {
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
