use std::cmp::{Ordering, PartialOrd};
use std::path;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Point {
    pub(super) row: usize,
    pub(super) column: usize,
}
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.row < other.row {
            Some(Ordering::Less)
        } else if self.row < other.row {
            Some(Ordering::Greater)
        } else {
            if self.column < other.column {
                Some(Ordering::Less)
            } else if self.column > other.column {
                Some(Ordering::Greater)
            } else {
                Some(Ordering::Equal)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Range {
    pub(super) start: Point,
    pub(super) end: Point,
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
pub(super) enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Feature {
    name: String,
    visibility: FeatureVisibility,
    range: Range,
}
impl Feature {
    pub(super) fn from_name_and_range<'a>(name: String, range: Range) -> Feature {
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
pub(super) struct Class {
    name: String,
    path: Option<Location>,
    features: Vec<Box<Feature>>,
    descendants: Vec<Box<Class>>,
    ancestors: Vec<Box<Class>>,
    range: Range,
}

impl Class {
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn features(&self) -> &Vec<Box<Feature>> {
        &self.features
    }
    pub(super) fn into_features(self) -> Vec<Box<Feature>> {
        self.features
    }
    pub(super) fn range(&self) -> &Range {
        &self.range
    }
    pub(super) fn location(&self) -> Option<&Location> {
        match &self.path {
            None => None,
            Some(file) => Some(&file),
        }
    }
    pub(super) fn from_name_range(name: String, range: Range) -> Class {
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

    pub(super) fn add_feature(&mut self, feature: &Feature) {
        self.features.push(Box::new(feature.clone()))
    }

    pub(super) fn add_location(&mut self, path: &PathBuf) {
        let path = path.clone();
        self.path = Some(Location { path })
    }
}
