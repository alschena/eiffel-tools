use anyhow::{anyhow, Context};
use async_lsp::lsp_types;
use gemini::lib::request::config::schema::{ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.row < other.row {
            Some(Ordering::Less)
        } else if other.row < self.row {
            Some(Ordering::Greater)
        } else {
            if self.column < other.column {
                Some(Ordering::Less)
            } else if other.column < self.column {
                Some(Ordering::Greater)
            } else {
                Some(Ordering::Equal)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}
impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (s, o) if s.start <= o.start && s.end >= o.end => Some(Ordering::Greater),
            (s, o) if s.start > o.start && s.end < o.end => Some(Ordering::Less),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Location {
    pub path: path::PathBuf,
}

impl From<&str> for Location {
    fn from(value: &str) -> Self {
        let path = value.into();
        Self { path }
    }
}
impl From<tree_sitter::Point> for Point {
    fn from(value: tree_sitter::Point) -> Self {
        Self {
            row: value.row,
            column: value.column,
        }
    }
}

impl From<tree_sitter::Range> for Range {
    fn from(value: tree_sitter::Range) -> Self {
        Self {
            start: value.start_point.into(),
            end: value.end_point.into(),
        }
    }
}
impl TryFrom<&Location> for lsp_types::Url {
    type Error = anyhow::Error;

    fn try_from(value: &Location) -> std::result::Result<Self, Self::Error> {
        Self::from_file_path(value.path.clone())
            .map_err(|()| anyhow!("code entitites location to url"))
    }
}
impl TryFrom<Point> for async_lsp::lsp_types::Position {
    type Error = anyhow::Error;

    fn try_from(value: Point) -> std::result::Result<Self, Self::Error> {
        let line = value.row.try_into().context("line conversion")?;
        let character = value.column.try_into().context("character conversion")?;
        Ok(Self { line, character })
    }
}
impl TryFrom<async_lsp::lsp_types::Position> for Point {
    type Error = anyhow::Error;

    fn try_from(value: async_lsp::lsp_types::Position) -> std::result::Result<Self, Self::Error> {
        let row = value.line.try_into().context("row conversion")?;
        let column = value.line.try_into().context("column conversion")?;
        Ok(Self { row, column })
    }
}
impl TryFrom<async_lsp::lsp_types::Range> for Range {
    type Error = anyhow::Error;

    fn try_from(value: async_lsp::lsp_types::Range) -> std::result::Result<Self, Self::Error> {
        let start = value.start.try_into().context("conversion of start")?;
        let end = value.end.try_into().context("conversion of end")?;
        Ok(Self { start, end })
    }
}
impl TryFrom<Range> for async_lsp::lsp_types::Range {
    type Error = anyhow::Error;

    fn try_from(value: Range) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            start: value.start.try_into()?,
            end: value.end.try_into()?,
        })
    }
}
