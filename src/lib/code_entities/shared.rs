use anyhow::{anyhow, Context};
use async_lsp::lsp_types;
use serde::Deserialize;
use std::cmp::{Ordering, PartialOrd};
use std::ops::Sub;
use std::path;
#[derive(Debug, PartialEq, Eq, Clone, Hash, Copy, Deserialize, Default)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}
impl Point {
    pub fn shift_left(&mut self, shift: usize) {
        assert!(shift <= self.column);
        self.column -= shift;
    }

    pub fn shift_right(&mut self, shift: usize) {
        self.column += shift;
    }

    pub fn reset_column(&mut self) {
        self.column = 0;
    }
}
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.row.cmp(&other.row) {
            Ordering::Less => Some(Ordering::Less),
            Ordering::Greater => Some(Ordering::Greater),
            _ => Some(self.column.cmp(&other.column)),
        }
    }
}
impl Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Self) -> Self::Output {
        let Point {
            row: lhs_row,
            column: lhs_column,
        } = self;

        let Point {
            row: rhs_row,
            column: rhs_column,
        } = rhs;

        Self::Output {
            row: lhs_row - rhs_row,
            column: if lhs_row == rhs_row {
                lhs_column - rhs_column
            } else {
                lhs_column
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

impl Range {
    pub fn new(start: Point, end: Point) -> Range {
        Range { start, end }
    }

    pub fn new_collapsed(point: Point) -> Range {
        Range::new(point, point)
    }

    pub fn contains(&self, point: Point) -> bool {
        self.start <= point && point <= self.end
    }

    pub fn collapse_to_line_start(&mut self) {
        self.start.reset_column();
        self.end.reset_column();
    }
}

impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (s, o) if s.start == o.start && s.end == o.end => Some(Ordering::Equal),
            (s, o) if s.start <= o.start && s.end >= o.end => Some(Ordering::Greater),
            (s, o) if s.start >= o.start && s.end <= o.end => Some(Ordering::Less),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Location(path::PathBuf);

impl Location {
    pub fn new(path: path::PathBuf) -> Location {
        Location(path)
    }
    pub fn path(&self) -> &path::Path {
        self.0.as_path()
    }
}

impl Location {
    pub fn to_lsp_location(&self, range: Range) -> Result<lsp_types::Location, anyhow::Error> {
        Ok(lsp_types::Location {
            uri: self.try_into()?,
            range: range.try_into()?,
        })
    }
}

impl TryFrom<&Location> for lsp_types::WorkspaceLocation {
    type Error = anyhow::Error;
    fn try_from(value: &Location) -> Result<Self, Self::Error> {
        match value.try_into() {
            Err(e) => Err(e),
            Ok(uri) => Ok(Self { uri }),
        }
    }
}

impl From<&str> for Location {
    fn from(value: &str) -> Self {
        let path = value.into();
        Self(path)
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
        Self::from_file_path(value.path()).map_err(|()| {
            anyhow!(
                "Fails to convert the code entitites location of path {:?} to the lsp-type Url",
                value.path()
            )
        })
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
        let column = value
            .character
            .try_into()
            .context("fails to convert character number into column")?;
        Ok(Self { row, column })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_points() {
        let start = Point { row: 1, column: 2 };
        let end = Point { row: 3, column: 1 };

        assert_eq!(
            end - start,
            Point { row: 2, column: 1 },
            "end - start == {:#?} - {:#?} == {:#?}",
            end,
            start,
            end - start
        )
    }
}
