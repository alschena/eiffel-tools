use crate::lib::code_entities::*;
use async_lsp::lsp_types::{DocumentSymbol, SymbolKind};

impl From<DocumentSymbol> for Feature<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_ne!(kind, SymbolKind::CLASS);
        Feature::from_name_and_range(name, range.into())
    }
}

impl From<DocumentSymbol> for Class<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_eq!(kind, SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v.into_iter().map(|x| x.into()).collect(),
            None => Vec::new(),
        };
        Class::from_name_range(name, range.into())
    }
}

impl From<async_lsp::lsp_types::Position> for Point {
    fn from(value: async_lsp::lsp_types::Position) -> Self {
        Self {
            row: value
                .line
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
            column: value
                .character
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
        }
    }
}

impl From<async_lsp::lsp_types::Range> for Range {
    fn from(value: async_lsp::lsp_types::Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}
