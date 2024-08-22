use crate::lib::code_entities::*;
use async_lsp::lsp_types::{DocumentSymbol, SymbolKind};

impl From<DocumentSymbol> for Feature<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        debug_assert_ne!(kind, SymbolKind::CLASS);
        Feature::from_name(name)
    }
}

impl From<DocumentSymbol> for Class<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        debug_assert_eq!(kind, SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v.into_iter().map(|x| x.into()).collect(),
            None => Vec::new(),
        };
        Class::from_name(name)
    }
}
