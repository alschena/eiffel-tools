use std::path::PathBuf;

#[derive(Debug)]
pub(super) enum FeatureVisibility<'a> {
    None,
    Some(&'a Class<'a>),
    All,
}

#[derive(Debug)]
pub(super) struct Feature<'a> {
    name: String,
    visibility: FeatureVisibility<'a>,
}

#[derive(Debug)]
pub(super) struct Class<'a> {
    name: String,
    path: PathBuf,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
}

impl Class<'_> {
    pub(crate) fn from_name_and_path<'a>(name: String, path: PathBuf) -> Class<'a> {
        let features = Vec::new();
        let descendants = Vec::new();
        let ancestors = Vec::new();
        Class {
            name,
            path,
            features,
            descendants,
            ancestors,
        }
    }
}
