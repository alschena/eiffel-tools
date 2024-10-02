use super::class::Class;
use super::shared::*;
use async_lsp::lsp_types;
use gemini::request::config::schema::{ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Feature {
    name: String,
    visibility: FeatureVisibility,
    range: Range,
}
impl Feature {
    pub fn from_name_and_range(name: String, range: Range) -> Feature {
        let visibility = FeatureVisibility::Private;
        Feature {
            name,
            visibility,
            range,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
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
