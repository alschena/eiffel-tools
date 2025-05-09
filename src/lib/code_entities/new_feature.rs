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

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Hash)]
pub struct FeatureName(String);
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
    params: HashMap<FeatureID, Parameters>,
    ret_type: HashMap<FeatureID, EiffelType>,
    notes: HashMap<FeatureID, Notes>,
    pre: HashMap<FeatureID, Precondition>,
    post: HashMap<FeatureID, Postcondition>,
    precondition_range: HashMap<FeatureID, Range>,
    postcondition_range: HashMap<FeatureID, Postcondition>,
    body_range: HashMap<FeatureID, Range>,
    vis: HashMap<FeatureID, FeatureVisibility>,
}

impl Features {
    pub fn name_of(&self, id: &FeatureID) -> &FeatureName {
        self.names
            .get(id)
            .unwrap_or_else(|| panic!("fails to find name of feature of id: {id:#?}"))
    }

    fn ranges<'slf, T: Iterator<Item = &'slf FeatureID>>(
        &'slf self,
        ids: T,
    ) -> impl Iterator<Item = &'slf Range> + use<'slf, T> {
        ids.map(|id| {
            self.ranges
                .get(id)
                .expect("fails to find range of feature of id: {id}")
        })
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
