use crate::lib::tree_sitter_extension::Parse;
use anyhow::{anyhow, Context};
use async_lsp::lsp_types;
use contract::{Block, Postcondition, Precondition};
use gemini::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use prelude::*;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::fmt::Display;
use std::path;
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tree_sitter::{Node, Query, QueryCursor};
mod class;
pub(crate) mod contract;
mod feature;
mod shared;
pub(crate) mod prelude {
    pub(crate) use super::class::Class;
    pub(crate) use super::contract;
    pub(crate) use super::feature::Feature;
    pub(crate) use super::shared::{Location, Point, Range};
    pub(crate) use super::{CodeEntity, Indent};
}
pub(crate) trait CodeEntity {}
pub(crate) trait Indent {
    const INDENTATION_LEVEL: u32;
    const INDENTATION_CHARACTER: char = '\t';
    fn indentation_string() -> String {
        (0..Self::INDENTATION_LEVEL)
            .into_iter()
            .fold(String::new(), |mut acc, _| {
                acc.push(Self::INDENTATION_CHARACTER);
                acc
            })
    }
}
pub(crate) trait ValidSyntax {
    fn valid_syntax(&self) -> bool;
}

#[cfg(test)]
mod tests {}
