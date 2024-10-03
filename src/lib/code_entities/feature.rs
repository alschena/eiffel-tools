use super::class::Class;
use super::*;
use crate::lib::tree_sitter;
use anyhow::anyhow;
use async_lsp::lsp_types;
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
    pub(super) preconditions: Option<Precondition>,
    pub(super) postconditions: Option<Postcondition>,
}
impl Feature {
    pub fn from_name_and_range(name: String, range: Range) -> Feature {
        Feature {
            name,
            visibility: FeatureVisibility::Private,
            range,
            preconditions: None,
            postconditions: None,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
}
impl<'a, 'b, 'c>
    TryFrom<(
        &::tree_sitter::Node<'b>,
        &mut ::tree_sitter::TreeCursor<'c>,
        &'a str,
    )> for Feature
where
    'b: 'c,
{
    type Error = anyhow::Error;

    fn try_from(
        (node, cursor, src): (
            &tree_sitter::Node<'b>,
            &mut tree_sitter::TreeCursor<'c>,
            &'a str,
        ),
    ) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "feature_declaration");
        cursor.reset(*node);
        let mut traversal = tree_sitter::WidthFirstTraversal::new(cursor);
        Ok(Feature {
            name: src[traversal
                .find(|x| x.kind() == "extended_feature_name")
                .ok_or(anyhow!(
                    "Each feature declaration contains an extended feature name"
                ))?
                .byte_range()]
            .into(),
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
            preconditions: None,
            postconditions: None,
        })
    }
}
impl TryFrom<lsp_types::DocumentSymbol> for Feature {
    type Error = anyhow::Error;

    fn try_from(value: lsp_types::DocumentSymbol) -> std::result::Result<Self, Self::Error> {
        let name = value.name;
        let kind = value.kind;
        let range = value.range.try_into()?;
        debug_assert_ne!(kind, lsp_types::SymbolKind::CLASS);
        Ok(Feature::from_name_and_range(name, range))
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
