use super::{processed_file::ProcessedFile, tree_sitter::WidthFirstTraversal};
use std::{path, str::FromStr};
use tree_sitter::Tree;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Point {
    pub(super) row: usize,
    pub(super) column: usize,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Range {
    pub(super) start: Point,
    pub(super) end: Point,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Location {
    pub(super) path: path::PathBuf,
}

impl From<&str> for Location {
    fn from(value: &str) -> Self {
        let path = value.into();
        Self { path }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum FeatureVisibility<'a> {
    Private,
    Some(&'a Class<'a>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Feature<'a> {
    name: String,
    visibility: FeatureVisibility<'a>,
    range: Range,
}
impl Feature<'_> {
    pub(super) fn from_name_and_range<'a>(name: String, range: Range) -> Feature<'a> {
        let visibility = FeatureVisibility::Private;
        Feature {
            name,
            visibility,
            range,
        }
    }
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn range(&self) -> &Range {
        &self.range
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Class<'a> {
    name: String,
    path: Option<Location>,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
    range: Range,
}

impl<'c> Class<'c> {
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn features(&self) -> &Vec<Feature<'_>> {
        &self.features
    }
    pub(super) fn range(&self) -> &Range {
        &self.range
    }
    pub(super) fn path(&self) -> Option<&Location> {
        match &self.path {
            None => None,
            Some(file) => Some(&file),
        }
    }
    pub(super) fn from_name_range(name: String, range: Range) -> Class<'c> {
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            path: None,
            features,
            descendants,
            ancestors,
            range,
        }
    }

    pub(super) fn add_feature(&mut self, feature: Feature<'c>) {
        self.features.push(feature)
    }

    pub(super) fn add_location(&mut self, file: &str) {
        self.path = Some(file.into())
    }
}
