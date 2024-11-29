use super::*;
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}
impl Point {
    pub fn shift_left(&mut self, shift: usize) {
        assert!(shift <= self.column);
        self.column = self.column - shift;
    }
    pub fn reset_column(&mut self) {
        self.column = 0;
    }
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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Range {
    start: Point,
    end: Point,
}
impl Range {
    pub fn new(start: Point, end: Point) -> Range {
        Range { start, end }
    }
    pub fn new_collapsed(point: Point) -> Range {
        Range::new(point.clone(), point)
    }
    pub fn start(&self) -> &Point {
        &self.start
    }
    pub fn end(&self) -> &Point {
        &self.end
    }
    pub fn collapse_to_line_start(&mut self) {
        self.start.reset_column();
        self.end.reset_column();
    }
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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Location {
    pub path: path::PathBuf,
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
        Self::from_file_path(value.path.clone()).map_err(|()| {
            anyhow!(
                "Fails to convert the code entitites location of path {:?} to the lsp-type Url",
                value.path
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
