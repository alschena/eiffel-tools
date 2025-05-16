use super::prelude::*;
use crate::lib::code_entities::feature::Notes;
use crate::lib::code_entities::feature::Parameters;
use crate::lib::code_entities::new_class::ClassID;
use contract::{Postcondition, Precondition};
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(ClassID),
    Public,
}

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct FeatureID(usize, usize);

impl<T> From<T> for FeatureID
where
    T: Into<(usize, usize)>,
{
    fn from(value: T) -> Self {
        let (f, s) = value.into();
        Self(f, s)
    }
}

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Hash)]
pub struct FeatureName(String);

impl<T: ToString> From<T> for FeatureName {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

impl Deref for FeatureName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Features {
    names: HashMap<FeatureID, FeatureName>,
    ranges: HashMap<FeatureID, Range>,
    classes: HashMap<FeatureID, ClassID>,
    parameters: HashMap<FeatureID, Parameters>,
    return_type: HashMap<FeatureID, EiffelType>,
    notes: HashMap<FeatureID, Notes>,
    preconditions: HashMap<FeatureID, Precondition>,
    postconditions: HashMap<FeatureID, Postcondition>,
    precondition_range: HashMap<FeatureID, Range>,
    postcondition_range: HashMap<FeatureID, Postcondition>,
    body_range: HashMap<FeatureID, Range>,
    visibility: HashMap<FeatureID, FeatureVisibility>,
}

impl Features {
    fn add_parameters(&mut self, id: FeatureID, params: Parameters) {
        self.parameters.insert(id, params);
    }

    fn add_return_type(&mut self, id: FeatureID, return_type: EiffelType) {
        self.return_type.insert(id, return_type);
    }

    pub fn name(&self, id: FeatureID) -> &FeatureName {
        self.names
            .get(&id)
            .unwrap_or_else(|| panic!("fails to find name of feature of id: {id:#?}"))
    }

    fn range(&self, id: FeatureID) -> &Range {
        self.ranges
            .get(&id)
            .expect("fails to find range of feature of id: {id}")
    }

    pub fn return_type(&self, id: FeatureID) -> Option<&EiffelType> {
        self.return_type.get(&id)
    }

    pub fn parameters(&self, id: FeatureID) -> Option<&Parameters> {
        self.parameters.get(&id)
    }

    fn feature_around_point<'slf, T: Iterator<Item = &'slf FeatureID>>(
        &'slf self,
        iter: T,
        point: Point,
    ) -> Option<&'slf FeatureID> {
        let mut iter = iter;
        iter.find(|id| self.ranges.get(id).is_some_and(|rng| rng.contains(point)))
    }
}

impl<T> From<T> for Features
where
    T: IntoIterator<Item = (FeatureID, FeatureName)>,
{
    fn from(value: T) -> Self {
        let names =
            HashMap::from_iter(value.into_iter().map(|(id, name)| (id.into(), name.into())));
        Features {
            names,
            ..Default::default()
        }
    }
}

pub fn base_name(base_features: &Features, feature_id: FeatureID) -> &FeatureName {
    base_features
        .names
        .get(&feature_id)
        .unwrap_or_else(|| panic!("fails to get name from feature ID: {:#?}", feature_id))
}

#[cfg(test)]
impl Features {
    pub fn mock_inheritance() -> Self {
        let id = FeatureID::from((2 as usize, 0 as usize));
        let name = FeatureName::from("grandparent_feature");
        Self::from([(id, name)])
    }

    /// Contains feature `f (x: INTEGER): BOOLEAN`
    pub fn mock_singleton() -> Self {
        let id = FeatureID::from((0 as usize, 0 as usize));
        let name = FeatureName::from("f");
        let mut fts = Self::from([(id, name)]);

        fts.add_parameters(id, Parameters::mock_integer("x"));
        fts.add_return_type(id, EiffelType::mock_boolean());
        fts
    }
}
